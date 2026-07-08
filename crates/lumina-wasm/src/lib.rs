//! WebAssembly bindings for the Lumina animation engine.
//!
//! Exposes [`LuminaEngine`] to JavaScript via `wasm-bindgen`: construct it
//! from scene JSON, call `render_frame(time)` for RGBA pixels to paint onto a
//! canvas, drive interactivity with `process_event`/`hit_test` (geometry-aware
//! hit-testing across all 17 object types, z-order respected), and load fonts
//! and images at runtime.
//!
//! Rendering uses the CPU (`tiny-skia`) backend compiled to wasm; running the
//! GPU backend in the browser via WebGPU is on the roadmap. Consumed by the
//! JavaScript SDK in `sdks/javascript`.

use lumina_core::{Event, EventBus, SceneGraph, Timeline};
use lumina_renderer::skia_backend::SkiaRenderer;
use lumina_renderer::Renderer;
use lumina_schema::{Object, Scene};
use serde_wasm_bindgen::{from_value, to_value};
use wasm_bindgen::prelude::*;

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
        let camera_state = self.timeline.get_camera_at(time, &self.scene);
        let camera = self.scene.camera.as_ref().map(|_| &camera_state);
        self.renderer.set_time(time);
        self.renderer
            .render_frame(
                &self.scene_graph.objects,
                &states,
                self.scene.canvas.width,
                self.scene.canvas.height,
                &self.scene.canvas.background,
                camera,
            )
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn process_event(&mut self, event: JsValue) -> Result<JsValue, JsValue> {
        let event: Event = from_value(event)?;
        // Returns an EventOutcome { actions, current_time, playing, emitted } so
        // the JS host can update its playhead and react to custom events.
        let outcome = self.event_bus.process_event(&event, &mut self.timeline);
        to_value(&outcome).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Returns the ID of the topmost object at pixel (x, y) at the given time,
    /// or null if no object is hit.
    pub fn hit_test(&self, x: f32, y: f32, time: f32) -> Option<String> {
        let states = self.timeline.get_state_at(time);

        let mut sorted_ids: Vec<_> = self.scene_graph.objects.keys().collect();
        sorted_ids.sort_by(|a, b| {
            let za = self.get_z_index(a);
            let zb = self.get_z_index(b);
            zb.cmp(&za) // higher z on top → tested first
        });

        for id in sorted_ids {
            if let Some(state) = states.get(id) {
                if self.is_point_in_object(id, x, y, state, &states) {
                    return Some(id.clone());
                }
            }
        }
        None
    }

    fn get_z_index(&self, id: &str) -> i32 {
        self.scene_graph
            .get_object(id)
            .map(|obj| match obj {
                Object::Circle(p) => p.z_index,
                Object::Rectangle(p) => p.z_index,
                Object::Polygon(p) => p.z_index,
                Object::Path(p) => p.z_index,
                Object::Line(p) => p.z_index,
                Object::Arrow(p) => p.z_index,
                Object::Text(p) => p.z_index,
                Object::LaTeX(p) => p.z_index,
                Object::Group(p) => p.z_index,
                Object::Image(p) => p.z_index,
                Object::SVG(p) => p.z_index,
                Object::NumberLine(p) => p.z_index,
                Object::Axes(p) => p.z_index,
                Object::Plot(p) => p.z_index,
                Object::BezierCurve(p) => p.z_index,
                Object::MathML(p) => p.z_index,
                Object::Particles(p) => p.z_index,
            })
            .unwrap_or(0)
    }

    fn is_point_in_object(
        &self,
        id: &str,
        px: f32,
        py: f32,
        state: &serde_json::Value,
        all_states: &std::collections::HashMap<String, serde_json::Value>,
    ) -> bool {
        match self.scene_graph.get_object(id) {
            // ── Circle ──────────────────────────────────────────────────────
            Some(Object::Circle(_)) => {
                let cx = state["cx"].as_f64().unwrap_or(0.0) as f32;
                let cy = state["cy"].as_f64().unwrap_or(0.0) as f32;
                let r = state["radius"].as_f64().unwrap_or(0.0) as f32;
                (px - cx).powi(2) + (py - cy).powi(2) <= r * r
            }

            // ── Rectangle ───────────────────────────────────────────────────
            Some(Object::Rectangle(_)) => {
                let ox = state["x"].as_f64().unwrap_or(0.0) as f32;
                let oy = state["y"].as_f64().unwrap_or(0.0) as f32;
                let w = state["width"].as_f64().unwrap_or(0.0) as f32;
                let h = state["height"].as_f64().unwrap_or(0.0) as f32;
                px >= ox && px <= ox + w && py >= oy && py <= oy + h
            }

            // ── Polygon — ray casting ────────────────────────────────────────
            Some(Object::Polygon(_)) => {
                let points = match state["points"].as_array() {
                    Some(p) => p,
                    None => return false,
                };
                point_in_polygon(px, py, points)
            }

            // ── Line — distance from segment ─────────────────────────────────
            Some(Object::Line(_)) => {
                let x1 = state["x1"].as_f64().unwrap_or(0.0) as f32;
                let y1 = state["y1"].as_f64().unwrap_or(0.0) as f32;
                let x2 = state["x2"].as_f64().unwrap_or(0.0) as f32;
                let y2 = state["y2"].as_f64().unwrap_or(0.0) as f32;
                let sw = state["stroke_width"].as_f64().unwrap_or(1.0) as f32;
                let tolerance = (sw / 2.0).max(4.0);
                point_to_segment_dist(px, py, x1, y1, x2, y2) <= tolerance
            }

            // ── Arrow — distance from shaft segment ──────────────────────────
            Some(Object::Arrow(_)) => {
                let from = state["from"].as_array();
                let to = state["to"].as_array();
                let (from, to) = match (from, to) {
                    (Some(f), Some(t)) if f.len() >= 2 && t.len() >= 2 => (f, t),
                    _ => return false,
                };
                let fx = from[0].as_f64().unwrap_or(0.0) as f32;
                let fy = from[1].as_f64().unwrap_or(0.0) as f32;
                let tx = to[0].as_f64().unwrap_or(0.0) as f32;
                let ty = to[1].as_f64().unwrap_or(0.0) as f32;
                let sw = state["stroke_width"].as_f64().unwrap_or(1.0) as f32;
                let tolerance = (sw / 2.0).max(4.0);
                point_to_segment_dist(px, py, fx, fy, tx, ty) <= tolerance
            }

            // ── BezierCurve — sample points and check segment proximity ──────
            Some(Object::BezierCurve(_)) => {
                let get_pt = |key: &str| -> Option<(f32, f32)> {
                    let arr = state[key].as_array()?;
                    Some((arr.first()?.as_f64()? as f32, arr.get(1)?.as_f64()? as f32))
                };
                let (x0, y0) = match get_pt("p0") {
                    Some(p) => p,
                    None => return false,
                };
                let (x1, y1) = match get_pt("p1") {
                    Some(p) => p,
                    None => return false,
                };
                let (x2, y2) = match get_pt("p2") {
                    Some(p) => p,
                    None => return false,
                };
                let (x3, y3) = match get_pt("p3") {
                    Some(p) => p,
                    None => return false,
                };
                let sw = state["stroke_width"].as_f64().unwrap_or(1.0) as f32;
                let tolerance = (sw / 2.0).max(4.0);
                point_near_cubic_bezier(
                    (px, py),
                    [(x0, y0), (x1, y1), (x2, y2), (x3, y3)],
                    tolerance,
                )
            }

            // ── Text / LaTeX / MathML — bounding box from font_size and content ─
            Some(Object::Text(_)) | Some(Object::LaTeX(_)) | Some(Object::MathML(_)) => {
                let x = state["x"].as_f64().unwrap_or(0.0) as f32;
                let y = state["y"].as_f64().unwrap_or(0.0) as f32;
                let font_size = state["font_size"].as_f64().unwrap_or(24.0) as f32;
                let content_len = state["content"]
                    .as_str()
                    .or_else(|| state["expression"].as_str())
                    .or_else(|| state["markup"].as_str())
                    .unwrap_or("")
                    .chars()
                    .count() as f32;
                // Approximate: each glyph ≈ 0.6 × font_size wide
                let w = content_len * font_size * 0.6;
                let h = font_size * 1.2;
                px >= x && px <= x + w && py >= y - h && py <= y
            }

            // ── Image / SVG — bounding box ───────────────────────────────────
            Some(Object::Image(_)) | Some(Object::SVG(_)) => {
                let x = state["x"].as_f64().unwrap_or(0.0) as f32;
                let y = state["y"].as_f64().unwrap_or(0.0) as f32;
                let w = state["width"].as_f64().unwrap_or(100.0) as f32;
                let h = state["height"].as_f64().unwrap_or(100.0) as f32;
                px >= x && px <= x + w && py >= y && py <= y + h
            }

            // ── Axes — bounding box of the drawn coordinate region ───────────
            Some(Object::Axes(_)) => {
                let x = state["x"].as_f64().unwrap_or(0.0) as f32;
                let y = state["y"].as_f64().unwrap_or(0.0) as f32;
                let x_range = state["x_range"].as_array();
                let y_range = state["y_range"].as_array();
                let scale = state["scale"].as_f64().unwrap_or(40.0) as f32;
                let x_min = x_range
                    .and_then(|a| a.first())
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0) as f32;
                let x_max = x_range
                    .and_then(|a| a.get(1))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(10.0) as f32;
                let y_min = y_range
                    .and_then(|a| a.first())
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0) as f32;
                let y_max = y_range
                    .and_then(|a| a.get(1))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(10.0) as f32;
                let ox = x + (0.0 - x_min) * scale;
                let oy = y - (0.0 - y_min) * scale;
                let left = ox + x_min * scale;
                let right = ox + x_max * scale;
                let top = oy - y_max * scale;
                let bottom = oy - y_min * scale;
                px >= left && px <= right && py >= top && py <= bottom
            }

            // ── NumberLine — bounding box of the line ────────────────────────
            Some(Object::NumberLine(_)) => {
                let x = state["x"].as_f64().unwrap_or(0.0) as f32;
                let y = state["y"].as_f64().unwrap_or(0.0) as f32;
                let length = state["length"].as_f64().unwrap_or(400.0) as f32;
                let tolerance = 10.0_f32;
                px >= x && px <= x + length && (py - y).abs() <= tolerance
            }

            // ── Plot — hit if inside the associated axes bounding box ─────────
            Some(Object::Plot(props)) => {
                let axes_id = state["axes_id"].as_str().unwrap_or(&props.axes_id);
                if let Some(axes_state) = all_states.get(axes_id) {
                    let ax = axes_state["x"].as_f64().unwrap_or(0.0) as f32;
                    let ay = axes_state["y"].as_f64().unwrap_or(0.0) as f32;
                    let x_range = axes_state["x_range"].as_array();
                    let y_range = axes_state["y_range"].as_array();
                    let scale = axes_state["scale"].as_f64().unwrap_or(40.0) as f32;
                    let x_min = x_range
                        .and_then(|a| a.first())
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0) as f32;
                    let x_max = x_range
                        .and_then(|a| a.get(1))
                        .and_then(|v| v.as_f64())
                        .unwrap_or(10.0) as f32;
                    let y_min = y_range
                        .and_then(|a| a.first())
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0) as f32;
                    let y_max = y_range
                        .and_then(|a| a.get(1))
                        .and_then(|v| v.as_f64())
                        .unwrap_or(10.0) as f32;
                    let ox = ax + (0.0 - x_min) * scale;
                    let oy = ay - (0.0 - y_min) * scale;
                    px >= ox + x_min * scale
                        && px <= ox + x_max * scale
                        && py >= oy - y_max * scale
                        && py <= oy - y_min * scale
                } else {
                    false
                }
            }

            // ── Group — check whether any child is hit ───────────────────────
            Some(Object::Group(props)) => {
                let gx = state["x"].as_f64().unwrap_or(0.0) as f32;
                let gy = state["y"].as_f64().unwrap_or(0.0) as f32;
                // Transform point into local space (ignoring scale/rotation for simplicity)
                let local_x = px - gx;
                let local_y = py - gy;
                props.children.iter().any(|cid| {
                    if let Some(child_state) = all_states.get(cid) {
                        self.is_point_in_object(cid, local_x, local_y, child_state, all_states)
                    } else {
                        false
                    }
                })
            }

            // ── Path — bounding box via d-string extents ─────────────────────
            Some(Object::Path(_)) => {
                // Parse path d to find bounding box
                let d = state["d"].as_str().unwrap_or("");
                if let Some((min_x, min_y, max_x, max_y)) = svg_path_bbox(d) {
                    let sw = state["stroke_width"].as_f64().unwrap_or(1.0) as f32;
                    let pad = sw / 2.0;
                    px >= min_x - pad && px <= max_x + pad && py >= min_y - pad && py <= max_y + pad
                } else {
                    false
                }
            }

            // ── Particles — bounding box around the emission radius ──────────
            Some(Object::Particles(_)) => {
                let ex = state["emitter_x"].as_f64().unwrap_or(0.0) as f32;
                let ey = state["emitter_y"].as_f64().unwrap_or(0.0) as f32;
                let lifetime = state["lifetime"].as_f64().unwrap_or(2.0) as f32;
                let speed = state["speed"].as_f64().unwrap_or(120.0) as f32;
                let reach = (lifetime * speed).max(1.0);
                px >= ex - reach && px <= ex + reach && py >= ey - reach && py <= ey + reach
            }

            None => false,
        }
    }

    pub fn load_font(&mut self, id: &str, data: &[u8]) -> Result<(), JsValue> {
        self.renderer
            .load_font(id, data)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Load a raster image (PNG/JPEG/WebP/GIF) or SVG asset. The host passes the
    /// raw bytes since WASM cannot read the filesystem.
    pub fn load_image(&mut self, id: &str, data: &[u8]) -> Result<(), JsValue> {
        self.renderer
            .load_image(id, data)
            .map_err(|e| JsValue::from_str(&e.to_string()))
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

// ── Geometry helpers ──────────────────────────────────────────────────────────

/// Ray-casting point-in-polygon test.
fn point_in_polygon(px: f32, py: f32, points: &[serde_json::Value]) -> bool {
    let n = points.len();
    if n < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let xi = points[i]
            .as_array()
            .and_then(|a| a.first())
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as f32;
        let yi = points[i]
            .as_array()
            .and_then(|a| a.get(1))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as f32;
        let xj = points[j]
            .as_array()
            .and_then(|a| a.first())
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as f32;
        let yj = points[j]
            .as_array()
            .and_then(|a| a.get(1))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as f32;
        if ((yi > py) != (yj > py)) && (px < (xj - xi) * (py - yi) / (yj - yi) + xi) {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Minimum distance from point (px,py) to line segment (x1,y1)→(x2,y2).
fn point_to_segment_dist(px: f32, py: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len_sq = dx * dx + dy * dy;
    if len_sq < f32::EPSILON {
        return ((px - x1).powi(2) + (py - y1).powi(2)).sqrt();
    }
    let t = ((px - x1) * dx + (py - y1) * dy) / len_sq;
    let t = t.clamp(0.0, 1.0);
    let nx = x1 + t * dx;
    let ny = y1 + t * dy;
    ((px - nx).powi(2) + (py - ny).powi(2)).sqrt()
}

/// Check if (px,py) is within `tolerance` pixels of a cubic Bézier curve by
/// sampling the curve and checking each segment.
fn point_near_cubic_bezier(p: (f32, f32), ctrl: [(f32, f32); 4], tolerance: f32) -> bool {
    let (px, py) = p;
    let [(x0, y0), (x1, y1), (x2, y2), (x3, y3)] = ctrl;
    const SAMPLES: usize = 32;
    let mut prev_x = x0;
    let mut prev_y = y0;
    for i in 1..=SAMPLES {
        let t = i as f32 / SAMPLES as f32;
        let u = 1.0 - t;
        let bx = u * u * u * x0 + 3.0 * u * u * t * x1 + 3.0 * u * t * t * x2 + t * t * t * x3;
        let by = u * u * u * y0 + 3.0 * u * u * t * y1 + 3.0 * u * t * t * y2 + t * t * t * y3;
        if point_to_segment_dist(px, py, prev_x, prev_y, bx, by) <= tolerance {
            return true;
        }
        prev_x = bx;
        prev_y = by;
    }
    false
}

/// Parse SVG path `d` attribute to extract an approximate bounding box.
fn svg_path_bbox(d: &str) -> Option<(f32, f32, f32, f32)> {
    let mut xs: Vec<f32> = Vec::new();
    let mut ys: Vec<f32> = Vec::new();
    let mut cur_x = 0.0_f32;
    let mut cur_y = 0.0_f32;

    let mut normalized = String::with_capacity(d.len() * 2);
    for ch in d.chars() {
        if ch.is_ascii_alphabetic() {
            normalized.push(' ');
            normalized.push(ch);
            normalized.push(' ');
        } else if ch == ',' {
            normalized.push(' ');
        } else {
            normalized.push(ch);
        }
    }

    let tokens: Vec<&str> = normalized.split_whitespace().collect();
    let mut i = 0;
    macro_rules! next_f32 {
        () => {{
            let v = tokens.get(i).and_then(|s| s.parse::<f32>().ok())?;
            i += 1;
            v
        }};
    }

    while i < tokens.len() {
        match tokens[i] {
            "M" => {
                i += 1;
                cur_x = next_f32!();
                cur_y = next_f32!();
                xs.push(cur_x);
                ys.push(cur_y);
            }
            "m" => {
                i += 1;
                cur_x += next_f32!();
                cur_y += next_f32!();
                xs.push(cur_x);
                ys.push(cur_y);
            }
            "L" => {
                i += 1;
                cur_x = next_f32!();
                cur_y = next_f32!();
                xs.push(cur_x);
                ys.push(cur_y);
            }
            "l" => {
                i += 1;
                cur_x += next_f32!();
                cur_y += next_f32!();
                xs.push(cur_x);
                ys.push(cur_y);
            }
            "H" => {
                i += 1;
                cur_x = next_f32!();
                xs.push(cur_x);
            }
            "h" => {
                i += 1;
                cur_x += next_f32!();
                xs.push(cur_x);
            }
            "V" => {
                i += 1;
                cur_y = next_f32!();
                ys.push(cur_y);
            }
            "v" => {
                i += 1;
                cur_y += next_f32!();
                ys.push(cur_y);
            }
            "C" => {
                i += 1;
                for _ in 0..2 {
                    next_f32!();
                    next_f32!();
                } // skip control points
                cur_x = next_f32!();
                cur_y = next_f32!();
                xs.push(cur_x);
                ys.push(cur_y);
            }
            "c" => {
                i += 1;
                for _ in 0..2 {
                    next_f32!();
                    next_f32!();
                }
                cur_x += next_f32!();
                cur_y += next_f32!();
                xs.push(cur_x);
                ys.push(cur_y);
            }
            "Z" | "z" => {
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }

    if xs.is_empty() || ys.is_empty() {
        return None;
    }
    let min_x = xs.iter().cloned().fold(f32::INFINITY, f32::min);
    let min_y = ys.iter().cloned().fold(f32::INFINITY, f32::min);
    let max_x = xs.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let max_y = ys.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    Some((min_x, min_y, max_x, max_y))
}
