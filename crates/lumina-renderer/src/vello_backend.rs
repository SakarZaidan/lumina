use crate::{Renderer, RendererError};
use lumina_schema::Object;
use serde_json::Value;
use std::collections::HashMap;
use vello::util::RenderContext;

pub struct VelloRenderer {
    #[allow(dead_code)]
    context: RenderContext,
}

impl VelloRenderer {
    pub async fn new() -> Result<Self, RendererError> {
        let context = RenderContext::new();
        Ok(Self { context })
    }
}

impl Renderer for VelloRenderer {
    fn render_frame(
        &mut self,
        _objects: &HashMap<String, Object>,
        _states: &HashMap<String, Value>,
        _width: u32,
        _height: u32,
        _background: &str,
    ) -> Result<Vec<u8>, RendererError> {
        Err(RendererError::Failed("Vello backend not fully implemented yet".to_string()))
    }

    fn load_font(&mut self, _id: &str, _data: &[u8]) -> Result<(), RendererError> {
        Err(RendererError::Failed("Vello backend not fully implemented yet".to_string()))
    }
}
