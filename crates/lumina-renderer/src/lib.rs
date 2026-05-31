pub(crate) mod raster;
pub mod skia_backend;
pub mod vello_backend;

use lumina_schema::{CameraState, Object};
use serde_json::Value;
use std::collections::HashMap;

pub trait Renderer {
    fn render_frame(
        &mut self,
        objects: &HashMap<String, Object>,
        states: &HashMap<String, Value>,
        width: u32,
        height: u32,
        background: &str,
        camera: Option<&CameraState>,
    ) -> Result<Vec<u8>, RendererError>;

    fn load_font(&mut self, id: &str, data: &[u8]) -> Result<(), RendererError>;

    /// Load a raster image (PNG/JPEG/WebP/GIF) or SVG asset by id. The bytes are
    /// decoded once and reused for every frame. Backends that cannot composite
    /// images (e.g. the GPU backend today) may treat this as a no-op.
    fn load_image(&mut self, _id: &str, _data: &[u8]) -> Result<(), RendererError> {
        Ok(())
    }

    /// Inform the renderer of the current timeline position (seconds) before the
    /// next `render_frame` call. Used to select the correct frame of an animated
    /// GIF asset. Default is a no-op for backends without time-dependent assets.
    fn set_time(&mut self, _time: f32) {}
}

#[derive(thiserror::Error, Debug)]
pub enum RendererError {
    #[error("Rendering failed: {0}")]
    Failed(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod renderer_tests;
