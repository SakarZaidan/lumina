//! Export pipeline for the Lumina animation engine.
//!
//! [`Exporter`] drives any [`luminafx_renderer::Renderer`] frame by frame and
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
use luminafx_core::{SceneGraph, Timeline};
use luminafx_renderer::Renderer;
use luminafx_schema::Scene;
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

    /// `ProRes` quantiser. Lower is better; the encoder's usable range is
    /// roughly 4 to 20, and it is not the same scale as either CRF.
    fn prores_qscale(self) -> u8 {
        match self {
            Quality::Draft => 17,
            Quality::Standard => 11,
            Quality::Final => 5,
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

/// Convert premultiplied 8-bit sRGB to premultiplied linear `f32`.
///
/// The transfer function is applied to the colour channels only. Alpha is a
/// coverage fraction, not a light measurement, so it is linear already and
/// gamma-decoding it would be wrong.
///
/// Premultiplied values are decoded as if they were straight: sRGB's transfer
/// function is not linear, so `decode(c * a) != decode(c) * a`, and a fully
/// correct conversion would un-multiply, decode, and re-multiply. That path
/// divides by an 8-bit alpha, which amplifies quantisation savagely at low
/// coverage — a single alpha step at `a = 1` scales the colour by 255. Given
/// the source is 8 bits either way, decoding in place is the smaller error and
/// the one that stays monotonic.
fn premultiplied_srgb8_to_linear_f32(rgba: &[u8]) -> Vec<f32> {
    /// sRGB electro-optical transfer function, on `[0, 1]`.
    fn to_linear(c: f32) -> f32 {
        if c <= 0.040_45 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }

    // 256 entries covers every possible input, so the transfer function is
    // evaluated 256 times per export rather than once per channel per pixel
    // per frame — 250 million times over a minute of 1080p.
    let lut: [f32; 256] = std::array::from_fn(|i| to_linear(i as f32 / 255.0));

    let mut out = Vec::with_capacity(rgba.len());
    let (pixels, _trailing) = rgba.as_chunks::<4>();
    for px in pixels {
        out.push(lut[px[0] as usize]);
        out.push(lut[px[1] as usize]);
        out.push(lut[px[2] as usize]);
        out.push(f32::from(px[3]) / 255.0);
    }
    out
}

/// Which alpha convention a consumer of rendered frames wants.
///
/// The renderer composes in premultiplied alpha. Most destinations want the
/// other convention, but not all of them do, and getting it wrong is silent:
/// at `a = 255` the two encodings are identical bytes, so an opaque scene
/// looks correct either way and only transparency reveals the mistake.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AlphaMode {
    /// Straight (non-premultiplied) — PNG, ffmpeg's `rgba` input, canvases.
    Straight,
    /// Premultiplied, which `OpenEXR` calls *associated* alpha and expects by
    /// default. Compositors read EXR that way, so handing them straight alpha
    /// makes every semi-transparent edge too bright.
    Premultiplied,
}

/// ffmpeg arguments for audio, split by position on the command line.
#[derive(Debug, Default)]
struct AudioArgs {
    /// Input-stage arguments: `-ss`/`-i` pairs, before the output options.
    inputs: Vec<String>,
    /// Output-stage arguments: the filter graph, stream maps, and codec.
    output: Vec<String>,
}

/// One audio file to mix into a video export, with its path already resolved.
///
/// The path is resolved *by the caller*, not from `scene.assets.audio`, and
/// that is the point. ffmpeg needs a filesystem path rather than bytes, so an
/// exporter that read the scene's own path strings would let any caller name
/// any file — and one of the callers is an HTTP server accepting scenes from
/// the network. The CLI resolves against the working directory; the server
/// resolves against `LUMINA_ASSET_ROOT` and rejects anything outside it. The
/// sandbox stays where the trust boundary is.
#[derive(Debug, Clone)]
pub struct AudioTrack {
    /// Resolved path to an audio file ffmpeg can decode.
    pub path: std::path::PathBuf,
    /// Seconds into the video at which the track starts. Negative values start
    /// the video part-way into the track.
    pub start: f32,
    /// Linear gain; `1.0` is the file as recorded.
    pub gain: f32,
}

impl AudioTrack {
    /// Build a track from a scene's [`luminafx_schema::AudioAsset`] and a path
    /// the caller has already resolved and authorised.
    #[must_use]
    pub fn new(path: std::path::PathBuf, asset: &luminafx_schema::AudioAsset) -> Self {
        Self {
            path,
            start: asset.start,
            gain: asset.gain,
        }
    }
}

/// Audio codec for a container, or `None` for containers that carry no sound.
#[derive(Debug, Clone, Copy)]
enum AudioCodec {
    /// MP4: AAC is the only codec every player is guaranteed to decode.
    Aac,
    /// `WebM`: the container admits Opus and Vorbis, and Opus is better at
    /// every bitrate.
    Opus,
    /// `MOV` intermediates: uncompressed, because a master should not carry a
    /// generation of lossy audio into whatever re-encodes it.
    Pcm,
}

impl AudioCodec {
    /// The `-c:a` value and any codec-specific arguments that follow it.
    fn args(self) -> &'static [&'static str] {
        match self {
            AudioCodec::Aac => &["-c:a", "aac", "-b:a", "192k"],
            AudioCodec::Opus => &["-c:a", "libopus", "-b:a", "128k"],
            AudioCodec::Pcm => &["-c:a", "pcm_s16le"],
        }
    }
}

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
    /// Audio to mix into video exports; see [`Exporter::set_audio`].
    audio: Vec<AudioTrack>,
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
        objects: &std::collections::HashMap<String, luminafx_schema::Object>,
        timeline: &Timeline,
        frame_idx: u32,
        out: &mut Vec<u8>,
        alpha: AlphaMode,
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
            if alpha == AlphaMode::Straight {
                luminafx_renderer::demultiply_in_place(out);
            }
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
        // This is the last point at which every export path is still holding
        // raw renderer output, so it is the one place the conversion can
        // happen — and it has to happen *after* the averaging above, which is
        // only correct on premultiplied values.
        if alpha == AlphaMode::Straight {
            luminafx_renderer::demultiply_in_place(out);
        }
        Ok(())
    }

    /// Wrap a renderer for export.
    pub fn new(renderer: R) -> Self {
        Self {
            renderer,
            audio: Vec::new(),
        }
    }

    /// Set the audio tracks mixed into subsequent video exports.
    ///
    /// The exporter deliberately does **not** read `scene.assets.audio`
    /// itself: see [`AudioTrack`] for why the path resolution belongs to the
    /// caller. Tracks are ignored by [`Exporter::export_png_sequence`] and by
    /// GIF export, neither of which has anywhere to put sound.
    pub fn set_audio(&mut self, tracks: Vec<AudioTrack>) {
        self.audio = tracks;
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
                    AlphaMode::Straight,
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
                AlphaMode::Straight,
            )?;

            sink(&frame_data)?;

            if frame_idx % 10 == 0 {
                log::info!("Rendered frame {frame_idx}/{total_frames}");
            }
        }
        Ok(())
    }

    /// ffmpeg arguments for the declared audio tracks, split by where they go.
    ///
    /// Two lists because ffmpeg's command line is positional: input options
    /// must precede the output-stage options, and the video's own encoder
    /// flags sit between them.
    fn build_audio_args(&self, codec: AudioCodec) -> Result<AudioArgs> {
        let mut inputs: Vec<String> = Vec::new();
        let mut chains: Vec<String> = Vec::new();
        let mut labels: Vec<String> = Vec::new();

        for (i, track) in self.audio.iter().enumerate() {
            let path = track.path.to_str().ok_or_else(|| {
                anyhow::anyhow!("audio path is not valid UTF-8: {:?}", track.path)
            })?;

            // A negative start means "the video begins part-way into this
            // track", which is a seek on the *input* rather than anything the
            // filter graph can express: filters cannot produce audio from
            // before the file started.
            if track.start < 0.0 {
                inputs.push("-ss".to_string());
                inputs.push(format!("{:.6}", -f64::from(track.start)));
            }
            inputs.push("-i".to_string());
            inputs.push(path.to_string());

            // Input 0 is the raw video pipe, so the tracks are 1..=n.
            let stream = i + 1;
            let mut chain = format!("[{stream}:a]");
            let mut stages: Vec<String> = Vec::new();
            if (track.gain - 1.0).abs() > f32::EPSILON {
                stages.push(format!("volume={:.6}", track.gain));
            }
            if track.start > 0.0 {
                // `adelay` takes milliseconds, and without `all=1` it delays
                // only the first channel — which turns a stereo track into one
                // channel of silence against one of sound.
                let ms = (f64::from(track.start) * 1000.0).round() as i64;
                stages.push(format!("adelay={ms}:all=1"));
            }
            if stages.is_empty() {
                // A filter chain may not be empty, and `anull` is the
                // documented way to say "pass through".
                stages.push("anull".to_string());
            }
            let label = format!("a{i}");
            chain.push_str(&stages.join(","));
            chain.push_str(&format!("[{label}]"));
            chains.push(chain);
            labels.push(label);
        }

        if labels.is_empty() {
            return Ok(AudioArgs::default());
        }

        // `normalize=0` keeps each track at the gain the scene asked for.
        // amix normalises by input count by default, so declaring a second
        // track would silently halve the volume of the first.
        let mixed = if labels.len() == 1 {
            format!("[{}]apad[aout]", labels[0])
        } else {
            format!(
                "{}amix=inputs={}:duration=longest:normalize=0,apad[aout]",
                labels.iter().map(|l| format!("[{l}]")).collect::<String>(),
                labels.len()
            )
        };
        chains.push(mixed);

        // `apad` above pads with silence indefinitely and `-shortest` then
        // cuts at the end of the video. Together they make the output exactly
        // as long as the animation, whether the audio is shorter or longer —
        // `-shortest` alone would truncate the video to a short track, and
        // neither alone handles both directions.
        let output = vec![
            "-filter_complex".to_string(),
            chains.join(";"),
            "-map".to_string(),
            "0:v".to_string(),
            "-map".to_string(),
            "[aout]".to_string(),
        ]
        .into_iter()
        .chain(codec.args().iter().map(|s| (*s).to_string()))
        .chain(std::iter::once("-shortest".to_string()))
        .collect();

        Ok(AudioArgs { inputs, output })
    }

    /// Render every frame as `frame_NNNN.exr` in `output_dir`, in linear light
    /// with associated (premultiplied) alpha.
    ///
    /// The intermediate for a compositor. EXR carries float channels and a
    /// documented colour space, so nothing downstream has to guess a transfer
    /// function or re-quantise on the way in.
    ///
    /// # What this does and does not buy
    ///
    /// It does **not** add information the renderer did not have. The CPU
    /// rasteriser has exactly one pixel type — 8-bit, sRGB — so these floats
    /// carry 8-bit values converted exactly, not extra precision
    /// (`AAA-OUT-01` in `plan/`, blocked on the rasteriser). What it buys is
    /// that nothing is lost *after* this point: no second quantisation, no
    /// guessed gamma, and alpha in the convention `OpenEXR` actually specifies.
    /// When the renderer gains a deeper buffer, this path carries it without
    /// changing.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be created or a frame cannot
    /// be written.
    pub fn export_exr_sequence(&mut self, scene: &Scene, output_dir: &Path) -> Result<()> {
        let scene_graph = SceneGraph::from_scene(scene);
        let timeline = Timeline::from_scene(scene);
        let total_frames = (scene.canvas.duration * scene.canvas.fps as f32).ceil() as u32;
        let (width, height) = (scene.canvas.width, scene.canvas.height);

        if !output_dir.exists() {
            std::fs::create_dir_all(output_dir)?;
        }

        // Same shape as the PNG sequence: render in order on this thread,
        // encode on a rayon pool. Encoding is the expensive half and needs no
        // renderer, and files carry their own frame number so the order they
        // are written in is not observable.
        let (tx, rx) =
            std::sync::mpsc::sync_channel::<(u32, Vec<u8>)>(png_queue_depth(width, height));
        let dir = output_dir.to_path_buf();

        let writer = std::thread::spawn(move || -> Result<()> {
            rx.into_iter().par_bridge().try_for_each(|(idx, data)| {
                let pixels = premultiplied_srgb8_to_linear_f32(&data);
                let img: ImageBuffer<image::Rgba<f32>, Vec<f32>> =
                    ImageBuffer::from_raw(width, height, pixels).ok_or_else(|| {
                        anyhow::anyhow!("Failed to create float image buffer from frame data")
                    })?;
                img.save(dir.join(format!("frame_{idx:04}.exr")))?;
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
                    // EXR specifies associated alpha, so the renderer's own
                    // convention is the one the format wants — no conversion,
                    // and none of its rounding.
                    AlphaMode::Premultiplied,
                )?;
                if tx
                    .send((frame_idx, std::mem::take(&mut frame_data)))
                    .is_err()
                {
                    break;
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
            .map_err(|_| anyhow::anyhow!("EXR encoder thread panicked"))?;
        write_result.context("writing EXR frames")?;
        render_result?;

        Ok(())
    }

    /// Pipe rendered frames into `FFmpeg` with the given output-stage arguments
    /// (everything after the rawvideo `-i -` input, up to the output path).
    fn encode_with_ffmpeg(
        &mut self,
        scene: &Scene,
        output_path: &Path,
        output_args: &[&str],
        audio: Option<AudioCodec>,
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

        // Audio inputs come after the video pipe, so the video is always
        // input 0 and the tracks are inputs 1..=n. `build_audio_args` relies
        // on that numbering.
        let audio_plan = audio
            .map(|codec| self.build_audio_args(codec))
            .transpose()?;
        if let Some(plan) = &audio_plan {
            args.extend(plan.inputs.iter().map(String::as_str));
        }

        args.extend_from_slice(output_args);

        if let Some(plan) = &audio_plan {
            args.extend(plan.output.iter().map(String::as_str));
        }
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
        self.encode_with_ffmpeg(scene, output_path, &args, Some(AudioCodec::Aac))
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
        self.encode_with_ffmpeg(scene, output_path, &args, Some(AudioCodec::Opus))
    }

    /// Export VP9 `WebM` **with an alpha channel**, at the default quality.
    ///
    /// # Errors
    ///
    /// Returns an error if ffmpeg is missing or the encode fails.
    pub fn export_webm_alpha(&mut self, scene: &Scene, output_path: &Path) -> Result<()> {
        self.export_webm_alpha_with(scene, output_path, Quality::default())
    }

    /// Export VP9 `WebM` with an alpha channel at a chosen [`Quality`].
    ///
    /// For compositing over other footage: give the scene a transparent
    /// `canvas.background` (`"#00000000"`) and whatever it draws arrives in an
    /// editor with its own edges rather than a rectangle of backdrop.
    ///
    /// The pixel format is fixed at 8-bit `yuva420p` rather than following
    /// `quality` — libvpx carries alpha in an auxiliary stream that has no
    /// 10-bit form, so `Quality::Final` buys a lower CRF here and not more
    /// bits. [`Exporter::export_mov_prores4444`] is the format to reach for
    /// when the depth matters.
    ///
    /// # Errors
    ///
    /// Returns an error if ffmpeg is missing or the encode fails.
    pub fn export_webm_alpha_with(
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
            "-row-mt",
            "1",
            "-pix_fmt",
            "yuva420p",
            // Without this libvpx writes the alpha plane but no player looks
            // for it; the file plays back opaque and the transparency is
            // silently gone.
            "-auto-alt-ref",
            "0",
        ];
        args.extend_from_slice(BT709_TAGS);
        self.encode_with_ffmpeg(scene, output_path, &args, Some(AudioCodec::Opus))
    }

    /// Export `ProRes` 4444 in a `MOV` container, at the default quality.
    ///
    /// # Errors
    ///
    /// Returns an error if ffmpeg is missing or the encode fails.
    pub fn export_mov_prores4444(&mut self, scene: &Scene, output_path: &Path) -> Result<()> {
        self.export_mov_prores4444_with(scene, output_path, Quality::default())
    }

    /// Export `ProRes` 4444 at a chosen [`Quality`] — the editorial master.
    ///
    /// 10-bit 4:4:4 with a full alpha channel: no chroma subsampling, so hard
    /// coloured edges and thin strokes survive intact where a 4:2:0 codec
    /// smears them, and it is the format every editor and compositor ingests
    /// without transcoding.
    ///
    /// `quality` maps to the encoder's quantiser rather than to a CRF. Files
    /// are large by design — this is an intermediate, not a delivery format.
    ///
    /// # Errors
    ///
    /// Returns an error if ffmpeg is missing or the encode fails.
    pub fn export_mov_prores4444_with(
        &mut self,
        scene: &Scene,
        output_path: &Path,
        quality: Quality,
    ) -> Result<()> {
        let qscale = quality.prores_qscale().to_string();
        let mut args = vec![
            "-c:v",
            "prores_ks",
            "-profile:v",
            "4444",
            "-pix_fmt",
            "yuva444p10le",
            // `ProRes` stores alpha at 16 bits; the default is to drop it.
            "-alpha_bits",
            "16",
            // QuickTime checks the vendor atom and refuses files written with
            // ffmpeg's default identifier. Every other `ProRes` writer claims
            // Apple's, and so must this one to be openable.
            "-vendor",
            "apl0",
            "-qscale:v",
            &qscale,
        ];
        args.extend_from_slice(BT709_TAGS);
        args.extend_from_slice(&["-movflags", "+faststart"]);
        self.encode_with_ffmpeg(scene, output_path, &args, Some(AudioCodec::Pcm))
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
            // GIF has no audio track to put anything in.
            None,
        )
    }
}

#[cfg(test)]
mod alpha_tests;
#[cfg(test)]
mod audio_tests;
#[cfg(test)]
mod export_tests;
#[cfg(test)]
mod exr_tests;
