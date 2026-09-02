//! Export pipeline for the Lumina animation engine.
//!
//! [`Exporter`] drives any [`lumina_renderer::Renderer`] frame by frame and
//! writes the result as:
//!
//! - a PNG frame sequence (via the `image` crate), or
//! - MP4 (H.264), `WebM` (VP9), or GIF (palette-based) by streaming raw RGBA
//!   frames to an **external `ffmpeg` binary** found on `PATH`.
//!
//! There is no in-process encoder: video export requires ffmpeg to be
//! installed, and fails with a descriptive error when it is missing.

// The engine has never contained `unsafe`, and the metric tracking that was a
// `grep` over the source — which by v0.4.0 was returning a false positive from
// the word appearing in a comment. `forbid` makes it a compile error instead:
// it cannot be silenced by an `allow` further down, so a future `unsafe` block
// has to be argued for by removing this line, in a diff a reviewer will see.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

use anyhow::{Context, Result};
use image::{ImageBuffer, Rgba};
use lumina_core::{SceneGraph, Timeline};
use lumina_renderer::Renderer;
use lumina_schema::Scene;
use rayon::prelude::*;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

/// Colour-space tags written into every video Lumina produces.
///
/// Without these a player has to *guess* how to interpret the pixels, and
/// players guess differently — the same file looks measurably different in
/// `QuickTime`, VLC and Chrome, usually as a shift in saturation and gamma.
///
/// Lumina renders in sRGB, whose primaries and transfer function are those of
/// Rec. 709, so tagging BT.709 is a statement of fact rather than a
/// conversion. `-color_range tv` matches the limited range that `yuv420p`
/// implies.
const BT709_TAGS: &[&str] = &[
    "-colorspace",
    "bt709",
    "-color_primaries",
    "bt709",
    "-color_trc",
    "bt709",
    "-color_range",
    "tv",
];

/// How much time to spend on encoding, and how much precision to keep.
///
/// Rendering is deterministic at every setting; this trades encoder effort and
/// bit depth, not pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Quality {
    /// Fast, larger files. For iterating on a scene.
    Draft,
    /// The default: visually lossless at a sane bitrate.
    #[default]
    Standard,
    /// Slow, 10-bit. For a master, or anything that will be re-encoded later —
    /// 10 bits keeps banding out of gradients that 8 bits introduces.
    Final,
}

impl Quality {
    /// x264 preset: the effort/size trade, not a quality trade.
    fn x264_preset(self) -> &'static str {
        match self {
            Quality::Draft => "veryfast",
            Quality::Standard => "medium",
            Quality::Final => "slow",
        }
    }

    /// H.264 constant-rate factor. Lower is better quality.
    fn crf_h264(self) -> u8 {
        match self {
            Quality::Draft => 23,
            Quality::Standard => 18,
            Quality::Final => 16,
        }
    }

    /// VP9 constant-rate factor; its scale differs from x264's.
    fn crf_vp9(self) -> u8 {
        match self {
            Quality::Draft => 36,
            Quality::Standard => 30,
            Quality::Final => 24,
        }
    }

    /// Pixel format for H.264. 10-bit at `Final` to keep gradients smooth.
    fn pix_fmt_h264(self) -> &'static str {
        match self {
            Quality::Final => "yuv420p10le",
            _ => "yuv420p",
        }
    }

    /// Pixel format for VP9.
    fn pix_fmt_vp9(self) -> &'static str {
        match self {
            Quality::Final => "yuv420p10le",
            _ => "yuv420p",
        }
    }
}

/// How many rendered frames may sit between the renderer and the encoder.
///
/// Deep enough to keep ffmpeg fed through a slow frame, shallow enough that
/// memory stays bounded: four 1080p frames is about 33 MB. Rendering blocks
/// when the queue is full, so a fast renderer throttles itself to the
/// encoder's rate instead of buffering the whole video.
const PIPELINE_DEPTH: usize = 4;

/// Memory the PNG encoder queue may hold, in bytes.
///
/// Sized in bytes rather than frames because a frame is not a fixed cost:
/// 32 frames is 265 MB at 1080p but over a gigabyte at 4K.
const PNG_QUEUE_BYTES: usize = 256 * 1024 * 1024;

/// How many frames of `width * height` fit in [`PNG_QUEUE_BYTES`].
///
/// Depth matters more here than for the ffmpeg pipeline because there are many
/// consumers rather than one: at a depth of 4 the pool never has more than four
/// frames to work on and most of it sits idle.
fn png_queue_depth(width: u32, height: u32) -> usize {
    let frame_bytes = (width as usize)
        .saturating_mul(height as usize)
        .saturating_mul(4)
        .max(1);
    (PNG_QUEUE_BYTES / frame_bytes).clamp(4, 64)
}

/// Drives a [`Renderer`] frame by frame to produce image sequences or
/// video files (video encoding is delegated to an external `ffmpeg`).
pub struct Exporter<R: Renderer> {
    renderer: R,
}

impl<R: Renderer> Exporter<R> {
    /// Render one frame, applying motion blur when the scene asks for it.
    ///
    /// With `motion_blur_samples == 1` this renders a single instant, exactly
    /// as before. Above that, the frame is rendered several times across the
    /// shutter interval and the results averaged, so anything moving smears
    /// the way a camera's shutter makes it.
    ///
    /// The samples are taken at fixed offsets centred on the frame's own
    /// instant, so the result is deterministic — a frame renders identically
    /// however many times it is asked for, which the whole engine depends on.
    ///
    /// Averaging happens on **premultiplied** values, which is what
    /// `render_frame` returns and what makes the average correct: averaging
    /// straight-alpha colours weights a nearly-transparent sample as heavily
    /// as an opaque one and produces haloes around moving edges.
    fn render_blurred(
        &mut self,
        scene: &Scene,
        objects: &std::collections::HashMap<String, lumina_schema::Object>,
        timeline: &Timeline,
        frame_idx: u32,
        out: &mut Vec<u8>,
    ) -> Result<()> {
        let fps = f64::from(scene.canvas.fps).max(1.0);
        let base = f64::from(frame_idx) / fps;
        let samples = scene.canvas.motion_blur_samples.max(1);

        let render_at = |renderer: &mut R, t: f64| -> Result<Vec<u8>> {
            let t = t as f32;
            let states = timeline.get_state_at(t);
            let camera_state = timeline.get_camera_at(t, scene);
            let camera = scene.camera.as_ref().map(|_| &camera_state);
            renderer.set_time(t);
            renderer
                .render_frame(
                    objects,
                    &states,
                    scene.canvas.width,
                    scene.canvas.height,
                    &scene.canvas.background,
                    camera,
                )
                .map_err(|e| anyhow::anyhow!(e))
        };

        if samples == 1 {
            *out = render_at(&mut self.renderer, base)?;
            return Ok(());
        }

        // The shutter is centred on the frame's instant rather than opening at
        // it, so a blurred frame stays aligned with the unblurred one — an
        // object is where the timeline says it is, smeared either side.
        let shutter = f64::from(scene.canvas.shutter).clamp(f64::EPSILON, 1.0) / fps;
        let mut accum: Vec<u32> = Vec::new();

        for k in 0..samples {
            let offset = ((f64::from(k) + 0.5) / f64::from(samples) - 0.5) * shutter;
            let frame = render_at(&mut self.renderer, base + offset)?;
            if accum.is_empty() {
                accum = vec![0u32; frame.len()];
            }
            for (a, v) in accum.iter_mut().zip(&frame) {
                *a += u32::from(*v);
            }
        }

        out.clear();
        out.reserve(accum.len());
        // Round to nearest rather than truncating: truncation biases every
        // channel down, which darkens a blurred frame relative to a sharp one.
        let half = samples / 2;
        out.extend(accum.iter().map(|a| ((a + half) / samples) as u8));
        Ok(())
    }

    /// Wrap a renderer for export.
    pub fn new(renderer: R) -> Self {
        Self { renderer }
    }

    /// Render every frame of `scene` as `frame_NNNN.png` files in
    /// `output_dir` (created if missing).
    pub fn export_png_sequence(&mut self, scene: &Scene, output_dir: &Path) -> Result<()> {
        let scene_graph = SceneGraph::from_scene(scene);
        let timeline = Timeline::from_scene(scene);
        let total_frames = (scene.canvas.duration * scene.canvas.fps as f32).ceil() as u32;

        if !output_dir.exists() {
            std::fs::create_dir_all(output_dir)?;
        }

        // Render here, compress on a rayon pool.
        //
        // Which half to parallelise was measured, not assumed. Rendering all
        // 1 560 frames of a 52-second 1080p scene took 2.3 s while the PNG
        // export took 4.5 s — so about half the time was compression, and
        // compression needs no renderer. Parallelising that half needs no
        // per-thread renderer, no font reloading, and no way for two threads to
        // disagree about a glyph.
        //
        // Files are independent and each carries its frame number, so the
        // order they are written in is not observable. Their *contents* come
        // from one renderer on one thread, in order, exactly as before.
        let (width, height) = (scene.canvas.width, scene.canvas.height);
        let (tx, rx) =
            std::sync::mpsc::sync_channel::<(u32, Vec<u8>)>(png_queue_depth(width, height));
        let dir = output_dir.to_path_buf();

        let writer = std::thread::spawn(move || -> Result<()> {
            rx.into_iter().par_bridge().try_for_each(|(idx, data)| {
                let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
                    ImageBuffer::from_raw(width, height, data).ok_or_else(|| {
                        anyhow::anyhow!("Failed to create image buffer from frame data")
                    })?;
                img.save(dir.join(format!("frame_{idx:04}.png")))?;
                Ok(())
            })
        });

        let mut frame_data = Vec::new();
        let render_result = (|| -> Result<()> {
            for frame_idx in 0..total_frames {
                self.render_blurred(
                    scene,
                    &scene_graph.objects,
                    &timeline,
                    frame_idx,
                    &mut frame_data,
                )?;

                if tx
                    .send((frame_idx, std::mem::take(&mut frame_data)))
                    .is_err()
                {
                    break; // encoders stopped; the join below reports why
                }
                if frame_idx % 10 == 0 {
                    log::info!("Rendered frame {}/{}", frame_idx + 1, total_frames);
                }
            }
            Ok(())
        })();
        drop(tx);

        let write_result = writer
            .join()
            .map_err(|_| anyhow::anyhow!("PNG encoder thread panicked"))?;
        write_result.context("writing PNG frames")?;
        render_result?;

        Ok(())
    }

    /// Render every frame in order, feeding each frame's RGBA bytes to `sink`.
    /// Shared by every FFmpeg-backed encoder so frame generation lives in one
    /// place.
    fn stream_frames<F: FnMut(&[u8]) -> Result<()>>(
        &mut self,
        scene: &Scene,
        mut sink: F,
    ) -> Result<()> {
        let scene_graph = SceneGraph::from_scene(scene);
        let timeline = Timeline::from_scene(scene);
        let total_frames = (scene.canvas.duration * scene.canvas.fps as f32).ceil() as u32;

        let mut frame_data = Vec::new();
        for frame_idx in 0..total_frames {
            self.render_blurred(
                scene,
                &scene_graph.objects,
                &timeline,
                frame_idx,
                &mut frame_data,
            )?;

            sink(&frame_data)?;

            if frame_idx % 10 == 0 {
                log::info!("Rendered frame {frame_idx}/{total_frames}");
            }
        }
        Ok(())
    }

    /// Pipe rendered frames into `FFmpeg` with the given output-stage arguments
    /// (everything after the rawvideo `-i -` input, up to the output path).
    fn encode_with_ffmpeg(
        &mut self,
        scene: &Scene,
        output_path: &Path,
        output_args: &[&str],
    ) -> Result<()> {
        let out = output_path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid output path"))?;
        let video_size = format!("{}x{}", scene.canvas.width, scene.canvas.height);
        let fps = scene.canvas.fps.to_string();

        let mut args: Vec<&str> = vec![
            "-y",
            "-f",
            "rawvideo",
            "-pixel_format",
            "rgba",
            "-video_size",
            &video_size,
            "-framerate",
            &fps,
            "-i",
            "-",
        ];
        args.extend_from_slice(output_args);
        args.push(out);

        let mut child = Command::new("ffmpeg")
            .args(&args)
            .stdin(Stdio::piped())
            .spawn()
            .context("Failed to spawn ffmpeg — is it installed and on PATH?")?;

        let mut stdin = child.stdin.take().context("Failed to open ffmpeg stdin")?;

        // Render and encode overlap instead of taking turns.
        //
        // The loop used to render a frame, write it to ffmpeg's stdin, and
        // only then render the next. Writing blocks once the pipe fills, so
        // the two stages effectively ran in sequence: a 52-second 1080p scene
        // measured 2.3 s of rendering plus ~6.6 s of encoding and took 12.9 s
        // end to end. Overlapping them bounds the total by the slower stage
        // instead of their sum, and encoding is by far the slower one.
        //
        // The channel is bounded, so memory stays capped at
        // PIPELINE_DEPTH frames (about 33 MB at 1080p) and rendering
        // self-throttles to whatever rate ffmpeg can consume rather than
        // racing ahead and buffering the whole video in RAM.
        //
        // Order is preserved by construction: one producer sends frames in
        // sequence and one consumer writes them in the order received. There
        // is no reordering for determinism to worry about.
        let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<u8>>(PIPELINE_DEPTH);

        let writer = std::thread::spawn(move || -> Result<()> {
            for frame in rx {
                stdin.write_all(&frame)?;
            }
            // Closing stdin is what tells ffmpeg the stream has ended.
            drop(stdin);
            Ok(())
        });

        let render_result = self.stream_frames(scene, |frame| {
            // A send error means the writer stopped — almost always because
            // ffmpeg died. Stop rendering and let the join below report why,
            // rather than rendering thousands more frames into a closed pipe.
            tx.send(frame.to_vec())
                .map_err(|_| anyhow::anyhow!("ffmpeg stopped accepting frames"))
        });
        drop(tx);

        let write_result = writer
            .join()
            .map_err(|_| anyhow::anyhow!("frame writer thread panicked"))?;

        // Report the writer's error first: when ffmpeg fails, the render side
        // only sees "the pipe closed", which is the symptom rather than the
        // cause.
        write_result.context("writing frames to ffmpeg")?;
        render_result?;

        let status = child.wait()?;
        if !status.success() {
            anyhow::bail!("FFmpeg exited with non-zero status: {status}");
        }
        log::info!("Export complete: {output_path:?}");
        Ok(())
    }

    /// Export H.264 MP4 — the broad-compatibility default.
    ///
    /// See [`BT709_TAGS`] for why the colour flags matter, and
    /// [`Exporter::export_mp4_with`] to choose quality.
    pub fn export_mp4(&mut self, scene: &Scene, output_path: &Path) -> Result<()> {
        self.export_mp4_with(scene, output_path, Quality::default())
    }

    /// Export H.264 MP4 at a chosen [`Quality`].
    ///
    /// # Errors
    ///
    /// Returns an error if ffmpeg is missing or the encode fails.
    pub fn export_mp4_with(
        &mut self,
        scene: &Scene,
        output_path: &Path,
        quality: Quality,
    ) -> Result<()> {
        let crf = quality.crf_h264().to_string();
        let mut args = vec![
            "-c:v",
            "libx264",
            "-preset",
            quality.x264_preset(),
            // x264 has a tune built for exactly this content: flat regions,
            // hard edges, and little sensor noise. It raises the deblocking
            // strength and adjusts psychovisual settings, which for rendered
            // animation is quality gained rather than traded.
            "-tune",
            "animation",
            "-crf",
            &crf,
            "-pix_fmt",
            quality.pix_fmt_h264(),
            // Tell libx264 to write the VUI itself.
            //
            // ffmpeg's generic `-color_primaries` reaches the H.264 VUI on
            // some builds and not others: Ubuntu's ffmpeg 6.1 writes it,
            // macOS and Windows ffmpeg 8.1 report `unknown` for the same
            // command line. The encoder always writes what it is told, so
            // saying it twice is the portable answer. The generic flags below
            // still carry the container-level metadata and the range.
            "-x264-params",
            "colorprim=bt709:transfer=bt709:colormatrix=bt709",
        ];
        args.extend_from_slice(BT709_TAGS);
        // Moves the index to the front so a browser can start playing before
        // the whole file has downloaded. Costs one extra pass over the output.
        args.extend_from_slice(&["-movflags", "+faststart"]);
        self.encode_with_ffmpeg(scene, output_path, &args)
    }

    /// Export VP9 `WebM` — smaller, web-friendly.
    pub fn export_webm(&mut self, scene: &Scene, output_path: &Path) -> Result<()> {
        self.export_webm_with(scene, output_path, Quality::default())
    }

    /// Export VP9 `WebM` at a chosen [`Quality`].
    ///
    /// # Errors
    ///
    /// Returns an error if ffmpeg is missing or the encode fails.
    pub fn export_webm_with(
        &mut self,
        scene: &Scene,
        output_path: &Path,
        quality: Quality,
    ) -> Result<()> {
        let crf = quality.crf_vp9().to_string();
        let mut args = vec![
            "-c:v",
            "libvpx-vp9",
            "-b:v",
            "0",
            "-crf",
            &crf,
            // VP9 is single-threaded per tile without this; on a wide frame it
            // is the difference between using one core and using several.
            "-row-mt",
            "1",
            "-pix_fmt",
            quality.pix_fmt_vp9(),
        ];
        args.extend_from_slice(BT709_TAGS);
        self.encode_with_ffmpeg(scene, output_path, &args)
    }

    /// Export an animated GIF using a single-pass palettegen/paletteuse filter
    /// graph (Floyd–Steinberg dithering) over the piped rawvideo stream.
    pub fn export_gif(&mut self, scene: &Scene, output_path: &Path) -> Result<()> {
        self.encode_with_ffmpeg(
            scene,
            output_path,
            &[
                "-vf",
                "split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=floyd_steinberg",
            ],
        )
    }
}

#[cfg(test)]
mod export_tests;
