use anyhow::{Context, Result};
use lumina_core::{SceneGraph, Timeline};
use lumina_renderer::Renderer;
use lumina_schema::Scene;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use image::{ImageBuffer, Rgba};

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

            let frame_data = self.renderer.render_frame(
                &scene_graph.objects,
                &states,
                scene.canvas.width,
                scene.canvas.height,
                &scene.canvas.background,
                camera,
            ).map_err(|e| anyhow::anyhow!(e))?;

            let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_raw(
                scene.canvas.width,
                scene.canvas.height,
                frame_data,
            ).ok_or_else(|| anyhow::anyhow!("Failed to create image buffer from frame data"))?;

            let filename = format!("frame_{:04}.png", frame_idx);
            img.save(output_dir.join(filename))?;

            log::info!("Exported frame {}/{}", frame_idx + 1, total_frames);
        }

        Ok(())
    }

    pub fn export_mp4(&mut self, scene: &Scene, output_path: &Path) -> Result<()> {
        let scene_graph = SceneGraph::from_scene(scene);
        let timeline = Timeline::from_scene(scene);
        let total_frames = (scene.canvas.duration * scene.canvas.fps as f32).ceil() as u32;

        let mut child = Command::new("ffmpeg")
            .args([
                "-y",
                "-f", "rawvideo",
                "-pixel_format", "rgba",
                "-video_size", &format!("{}x{}", scene.canvas.width, scene.canvas.height),
                "-framerate", &scene.canvas.fps.to_string(),
                "-i", "-",
                "-c:v", "libx264",
                "-preset", "fast",
                "-crf", "18",
                "-pix_fmt", "yuv420p",
                output_path.to_str().ok_or_else(|| anyhow::anyhow!("Invalid output path"))?,
            ])
            .stdin(Stdio::piped())
            .spawn()
            .context("Failed to spawn ffmpeg — is it installed and on PATH?")?;

        let mut stdin = child.stdin.take().context("Failed to open ffmpeg stdin")?;

        for frame_idx in 0..total_frames {
            let time = frame_idx as f32 / scene.canvas.fps as f32;
            let states = timeline.get_state_at(time);
            let camera_state = timeline.get_camera_at(time, scene);
            let camera = scene.camera.as_ref().map(|_| &camera_state);

            let frame_data = self.renderer.render_frame(
                &scene_graph.objects,
                &states,
                scene.canvas.width,
                scene.canvas.height,
                &scene.canvas.background,
                camera,
            ).map_err(|e| anyhow::anyhow!(e))?;

            stdin.write_all(&frame_data)?;

            if frame_idx % 10 == 0 {
                log::info!("Rendered frame {}/{}", frame_idx, total_frames);
            }
        }

        drop(stdin);
        let status = child.wait()?;
        if !status.success() {
            anyhow::bail!("FFmpeg exited with non-zero status: {}", status);
        }

        log::info!("MP4 export complete: {:?}", output_path);
        Ok(())
    }
}

#[cfg(test)]
mod export_tests;
