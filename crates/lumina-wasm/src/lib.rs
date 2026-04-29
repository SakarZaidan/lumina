use wasm_bindgen::prelude::*;
use lumina_core::{SceneGraph, Timeline, Event, EventBus};
use lumina_renderer::Renderer;
use lumina_renderer::skia_backend::SkiaRenderer;
use lumina_schema::{Scene, Object};
use serde_wasm_bindgen::{from_value, to_value};

#[wasm_bindgen]
pub struct LuminaEngine {
    scene: Scene,
    timeline: Timeline,
    scene_graph: SceneGraph,
    renderer: SkiaRenderer,
    event_bus: EventBus,
}

#[wasm_bindgen]
impl LuminaEngine {
    #[wasm_bindgen(constructor)]
    pub fn new(scene_json: JsValue) -> Result<LuminaEngine, JsValue> {
        console_error_panic_hook::set_once();
        let scene: Scene = from_value(scene_json)?;
        let timeline = Timeline::from_scene(&scene);
        let scene_graph = SceneGraph::from_scene(&scene);
        let renderer = SkiaRenderer::new();
        let event_bus = EventBus::new(&scene);

        Ok(LuminaEngine {
            scene,
            timeline,
            scene_graph,
            renderer,
            event_bus,
        })
    }

    pub fn render_frame(&mut self, time: f32) -> Result<Vec<u8>, JsValue> {
        let states = self.timeline.get_state_at(time);
        let frame_data = self.renderer.render_frame(
            &self.scene_graph.objects,
            &states,
            self.scene.canvas.width,
            self.scene.canvas.height,
        ).map_err(|e| JsValue::from_str(&e.to_string()))?;

        Ok(frame_data)
    }

    pub fn process_event(&mut self, event: JsValue) -> Result<JsValue, JsValue> {
        let event: Event = from_value(event)?;
        let actions = self.event_bus.process_event(&event, &mut self.timeline);
        Ok(to_value(&actions)?)
    }

    pub fn hit_test(&self, x: f32, y: f32, time: f32) -> Option<String> {
        let states = self.timeline.get_state_at(time);
        
        // Simple hit testing for MVP
        // Sort objects by z-index descending
        let mut sorted_ids: Vec<_> = self.scene_graph.objects.keys().collect();
        sorted_ids.sort_by(|a, b| {
            let z_a = self.get_z_index(a);
            let z_b = self.get_z_index(b);
            z_b.cmp(&z_a)
        });

        for id in sorted_ids {
            if let Some(state) = states.get(id) {
                if self.is_point_in_object(id, x, y, state) {
                    return Some(id.clone());
                }
            }
        }
        None
    }

    fn get_z_index(&self, id: &str) -> i32 {
        if let Some(obj) = self.scene_graph.get_object(id) {
            match obj {
                Object::Circle(p) => p.z_index,
                Object::Rectangle(p) => p.z_index,
                Object::Polygon(p) => p.z_index,
                Object::Path(p) => p.z_index,
                Object::Line(p) => p.z_index,
                Object::Arrow(p) => p.z_index,
                Object::Text(p) => p.z_index,
                Object::LaTeX(p) => p.z_index,
                Object::Group(p) => p.z_index,
            }
        } else {
            0
        }
    }

    fn is_point_in_object(&self, id: &str, px: f32, py: f32, state: &serde_json::Value) -> bool {
        // Simplified hit test for MVP
        if let Some(obj) = self.scene_graph.get_object(id) {
            match obj {
                Object::Circle(_) => {
                    let cx = state["cx"].as_f64().unwrap_or(0.0) as f32;
                    let cy = state["cy"].as_f64().unwrap_or(0.0) as f32;
                    let r = state["radius"].as_f64().unwrap_or(0.0) as f32;
                    let dx = px - cx;
                    let dy = py - cy;
                    (dx * dx + dy * dy) <= (r * r)
                }
                Object::Rectangle(_) => {
                    let ox = state["x"].as_f64().unwrap_or(0.0) as f32;
                    let oy = state["y"].as_f64().unwrap_or(0.0) as f32;
                    let w = state["width"].as_f64().unwrap_or(0.0) as f32;
                    let h = state["height"].as_f64().unwrap_or(0.0) as f32;
                    px >= ox && px <= ox + w && py >= oy && py <= oy + h
                }
                _ => false // Other types not yet implemented for hit test
            }
        } else {
            false
        }
    }

    pub fn load_font(&mut self, id: &str, data: &[u8]) -> Result<(), JsValue> {
        self.renderer.load_font(id, data).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn duration(&self) -> f32 {
        self.scene.canvas.duration
    }

    pub fn width(&self) -> u32 {
        self.scene.canvas.width
    }

    pub fn height(&self) -> u32 {
        self.scene.canvas.height
    }
}
