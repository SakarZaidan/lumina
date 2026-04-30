pub mod skia_backend;
pub mod vello_backend;

use lumina_schema::Object;
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
    ) -> Result<Vec<u8>, RendererError>;

    fn load_font(&mut self, id: &str, data: &[u8]) -> Result<(), RendererError>;
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
