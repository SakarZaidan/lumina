//! Export pipeline for the Lumina animation engine.
//!
//! [`Exporter`] drives any [`lumina_renderer::Renderer`] frame by frame and
//! writes the result as:
//!
//! - a PNG frame sequence (via the `image` crate), or
//! - MP4 (H.264), WebM (VP9), or GIF (palette-based) by streaming raw RGBA
//!   frames to an **external `ffmpeg` binary** found on `PATH`.
//!
//! There is no in-process encoder: video export requires ffmpeg to be
//! installed, and fails with a descriptive error when it is missing.

use anyhow::{Context, Result};
use image::{ImageBuffer, Rgba};
use lumina_core::{SceneGraph, Timeline};
use lumina_renderer::Renderer;
use lumina_schema::Scene;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

pub struct Exporter<R: Renderer> {
    renderer: R,
}

impl<R: Renderer> Exporter<R> {
    pub fn new(renderer: R) -> Self {
        Self { renderer }
    }

    pub fn export_png_sequence(&mut self, scene: &Scene, output_dir: &Path) -> Result<()> {
        let scene_graph = SceneGraph::from_scene(scene);
        let timeline = Timeline::from_scene(scene);
        let total_frames = (scene.canvas.duration * scene.canvas.fps as f32).ceil() as u32;

        if !output_dir.exists() {
            std::fs::create_dir_all(output_dir)?;
        }

        for frame_idx in 0..total_frames {
            let time = frame_idx as f32 / scene.canvas.fps as f32;
            let states = timeline.get_state_at(time);
            let camera_state = timeline.get_camera_at(time, scene);
            let camera = scene.camera.as_ref().map(|_| &camera_state);

            self.renderer.set_time(time);
            let frame_data = self
                .renderer
                .render_frame(
                    &scene_graph.objects,
                    &states,
                    scene.canvas.width,
                    scene.canvas.height,
                    &scene.canvas.background,
                    camera,
                )
                .map_err(|e| anyhow::anyhow!(e))?;

            let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
                ImageBuffer::from_raw(scene.canvas.width, scene.canvas.height, frame_data)
                    .ok_or_else(|| {
                        anyhow::anyhow!("Failed to create image buffer from frame data")
                    })?;

            let filename = format!("frame_{:04}.png", frame_idx);
            img.save(output_dir.join(filename))?;

            log::info!("Exported frame {}/{}", frame_idx + 1, total_frames);
        }

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

        for frame_idx in 0..total_frames {
            let time = frame_idx as f32 / scene.canvas.fps as f32;
            let states = timeline.get_state_at(time);
            let camera_state = timeline.get_camera_at(time, scene);
            let camera = scene.camera.as_ref().map(|_| &camera_state);

            self.renderer.set_time(time);
            let frame_data = self
                .renderer
                .render_frame(
                    &scene_graph.objects,
                    &states,
                    scene.canvas.width,
                    scene.canvas.height,
                    &scene.canvas.background,
                    camera,
                )
                .map_err(|e| anyhow::anyhow!(e))?;

            sink(&frame_data)?;

            if frame_idx % 10 == 0 {
                log::info!("Rendered frame {}/{}", frame_idx, total_frames);
            }
        }
        Ok(())
    }

    /// Pipe rendered frames into FFmpeg with the given output-stage arguments
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
        self.stream_frames(scene, |frame| stdin.write_all(frame).map_err(Into::into))?;

        drop(stdin);
        let status = child.wait()?;
        if !status.success() {
            anyhow::bail!("FFmpeg exited with non-zero status: {}", status);
        }
        log::info!("Export complete: {:?}", output_path);
        Ok(())
    }

    /// Export H.264 MP4 (`-crf 18`, yuv420p) — broad-compatibility default.
    pub fn export_mp4(&mut self, scene: &Scene, output_path: &Path) -> Result<()> {
        self.encode_with_ffmpeg(
            scene,
            output_path,
            &[
                "-c:v", "libx264", "-preset", "fast", "-crf", "18", "-pix_fmt", "yuv420p",
            ],
        )
    }

    /// Export VP9 WebM (`-crf 30 -b:v 0`, yuv420p) — smaller, web-friendly.
    pub fn export_webm(&mut self, scene: &Scene, output_path: &Path) -> Result<()> {
        self.encode_with_ffmpeg(
            scene,
            output_path,
            &[
                "-c:v",
                "libvpx-vp9",
                "-b:v",
                "0",
                "-crf",
                "30",
                "-pix_fmt",
                "yuv420p",
            ],
        )
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
