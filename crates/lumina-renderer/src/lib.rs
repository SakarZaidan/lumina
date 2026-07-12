//! Rendering backends for the Lumina animation engine.
//!
//! Exposes the [`Renderer`] trait — "given objects and their animated states
//! at one instant, produce an RGBA frame" — and two implementations:
//!
//! - [`skia_backend::SkiaRenderer`] — CPU rasterizer over `tiny-skia`. The
//!   reference backend: all 17 object types, gradients, drop shadows,
//!   rounded rectangles, dashed strokes, SVG (via `resvg`) and animated-GIF
//!   compositing.
//! - [`vello_backend::VelloRenderer`] — GPU backend over `vello`/`wgpu`
//!   (headless). Full feature parity with the CPU backend: all object
//!   types, gradients, drop shadows, rounded rectangles and
//!   `draw_fraction` reveal (see the backend-parity table in the book's
//!   architecture chapter).
//!
//! Both backends share glyph/particle rasterization (`raster`) and all
//! parsing/geometry/ordering decisions (`common`), verified by the
//! cross-backend pixel-diff suite in `tests/backend_parity.rs`. Rendering
//! is deterministic: the same inputs always produce the same frame.

#![warn(missing_docs)]

pub(crate) mod common;
pub(crate) mod raster;
/// CPU reference backend (`tiny-skia`).
pub mod skia_backend;
/// GPU backend (`vello`/`wgpu`, headless).
pub mod vello_backend;

use lumina_schema::{CameraState, Object};
use serde_json::Value;
use std::collections::HashMap;

/// A rendering backend: "given objects and their animated states at one
/// instant, produce an RGBA frame".
pub trait Renderer {
    /// Render one frame to tightly packed RGBA8 (`width * height * 4`
    /// bytes). `states` carries each object's animated property values at
    /// the current time; `camera` applies a pan/zoom root transform.
    fn render_frame(
        &mut self,
        objects: &HashMap<String, Object>,
        states: &HashMap<String, Value>,
        width: u32,
        height: u32,
        background: &str,
        camera: Option<&CameraState>,
    ) -> Result<Vec<u8>, RendererError>;

    /// Load a TTF/OTF font under `id` for text objects to reference.
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

/// Errors surfaced by a [`Renderer`].
#[derive(thiserror::Error, Debug)]
pub enum RendererError {
    /// The backend could not produce a frame (adapter/device loss,
    /// allocation failure, malformed input).
    #[error("Rendering failed: {0}")]
    Failed(String),
    /// An underlying I/O operation failed.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod renderer_tests;
