use crate::{Renderer, RendererError};
use lumina_schema::{Object, TextProps};
use lumina_text::TextEngine;
use serde_json::Value;
use std::collections::HashMap;
use tiny_skia::*;

pub struct SkiaRenderer {
    text_engine: TextEngine,
}

impl SkiaRenderer {
    pub fn new() -> Self {
        Self {
            text_engine: TextEngine::new(),
        }
    }

    fn get_root_objects(&self, objects: &HashMap<String, Object>) -> Vec<String> {
        let mut child_ids = std::collections::HashSet::new();
        for obj in objects.values() {
            if let Object::Group(group) = obj {
                for child_id in &group.children {
                    child_ids.insert(child_id.clone());
                }
            }
        }

        let mut roots = Vec::new();
        for id in objects.keys() {
            if !child_ids.contains(id) {
                roots.push(id.clone());
            }
        }
        roots
    }

    fn draw_node(
        &self,
        pixmap: &mut Pixmap,
        id: &str,
        objects: &HashMap<String, Object>,
        states: &HashMap<String, Value>,
        parent_transform: Transform,
    ) -> Result<(), RendererError> {
        let obj = objects.get(id).ok_or_else(|| RendererError::Failed(format!("Object {} not found", id)))?;
        let state = states.get(id).ok_or_else(|| RendererError::Failed(format!("State for {} not found", id)))?;

        match obj {
            Object::Group(props) => {
                let x = state["x"].as_f64().unwrap_or(0.0) as f32;
                let y = state["y"].as_f64().unwrap_or(0.0) as f32;
                let scale = state["scale"].as_f64().unwrap_or(1.0) as f32;
                let rotation = state["rotation"].as_f64().unwrap_or(0.0) as f32;

                let mut transform = parent_transform;
                transform = transform.pre_translate(x, y);
                transform = transform.pre_scale(scale, scale);
                transform = transform.pre_rotate(rotation.to_degrees());

                for child_id in &props.children {
                    self.draw_node(pixmap, child_id, objects, states, transform)?;
                }
            }
            _ => {
                self.draw_leaf_object(pixmap, obj, state, parent_transform)?;
            }
        }
        Ok(())
    }

    fn draw_leaf_object(
        &self,
        pixmap: &mut Pixmap,
        obj: &Object,
        state: &Value,
        transform: Transform,
    ) -> Result<(), RendererError> {
        match obj {
            Object::Circle(_) => {
                let cx = state["cx"].as_f64().unwrap_or(0.0) as f32;
                let cy = state["cy"].as_f64().unwrap_or(0.0) as f32;
                let radius = state["radius"].as_f64().unwrap_or(0.0) as f32;
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;
                let fill_color = parse_color(state["fill"].as_str().unwrap_or("#FFFFFF"), opacity);

                let mut pb = PathBuilder::new();
                pb.push_circle(cx, cy, radius);
                if let Some(path) = pb.finish() {
                    let mut paint = Paint::default();
                    paint.set_color(fill_color);
                    paint.anti_alias = true;
                    pixmap.fill_path(&path, &paint, FillRule::Winding, transform, None);
                }
            }
            Object::Rectangle(_) => {
                let x = state["x"].as_f64().unwrap_or(0.0) as f32;
                let y = state["y"].as_f64().unwrap_or(0.0) as f32;
                let width = state["width"].as_f64().unwrap_or(0.0) as f32;
                let height = state["height"].as_f64().unwrap_or(0.0) as f32;
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;
                let fill_color = parse_color(state["fill"].as_str().unwrap_or("#FFFFFF"), opacity);

                if let Some(rect) = Rect::from_xywh(x, y, width, height) {
                    let mut paint = Paint::default();
                    paint.set_color(fill_color);
                    paint.anti_alias = true;
                    pixmap.fill_rect(rect, &paint, transform, None);
                }
            }
            Object::Polygon(_) => {
                let points = state["points"].as_array().unwrap();
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;
                let fill_color = parse_color(state["fill"].as_str().unwrap_or("#FFFFFF"), opacity);

                let mut pb = PathBuilder::new();
                for (i, p) in points.iter().enumerate() {
                    let p = p.as_array().unwrap();
                    let x = p[0].as_f64().unwrap() as f32;
                    let y = p[1].as_f64().unwrap() as f32;
                    if i == 0 {
                        pb.move_to(x, y);
                    } else {
                        pb.line_to(x, y);
                    }
                }
                pb.close();

                if let Some(path) = pb.finish() {
                    let mut paint = Paint::default();
                    paint.set_color(fill_color);
                    paint.anti_alias = true;
                    pixmap.fill_path(&path, &paint, FillRule::Winding, transform, None);
                }
            }
            Object::Line(_) => {
                let x1 = state["x1"].as_f64().unwrap_or(0.0) as f32;
                let y1 = state["y1"].as_f64().unwrap_or(0.0) as f32;
                let x2 = state["x2"].as_f64().unwrap_or(0.0) as f32;
                let y2 = state["y2"].as_f64().unwrap_or(0.0) as f32;
                let stroke_width = state["stroke_width"].as_f64().unwrap_or(1.0) as f32;
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;
                let stroke_color = parse_color(state["stroke"].as_str().unwrap_or("#FFFFFF"), opacity);

                let mut pb = PathBuilder::new();
                pb.move_to(x1, y1);
                pb.line_to(x2, y2);
                if let Some(path) = pb.finish() {
                    let mut paint = Paint::default();
                    paint.set_color(stroke_color);
                    paint.anti_alias = true;
                    let mut stroke = Stroke::default();
                    stroke.width = stroke_width;
                    pixmap.stroke_path(&path, &paint, &stroke, transform, None);
                }
            }
            Object::Arrow(_) => {
                let from = state["from"].as_array().unwrap();
                let to = state["to"].as_array().unwrap();
                let fx = from[0].as_f64().unwrap() as f32;
                let fy = from[1].as_f64().unwrap() as f32;
                let tx = to[0].as_f64().unwrap() as f32;
                let ty = to[1].as_f64().unwrap() as f32;
                let stroke_width = state["stroke_width"].as_f64().unwrap_or(1.0) as f32;
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;
                let color = parse_color(state["color"].as_str().unwrap_or("#FFFFFF"), opacity);

                let mut pb = PathBuilder::new();
                pb.move_to(fx, fy);
                pb.line_to(tx, ty);

                let dx = tx - fx;
                let dy = ty - fy;
                let angle = dy.atan2(dx);
                let head_len = 10.0;
                let head_angle = std::f32::consts::PI / 6.0;

                pb.move_to(tx, ty);
                pb.line_to(tx - head_len * (angle - head_angle).cos(), ty - head_len * (angle - head_angle).sin());
                pb.move_to(tx, ty);
                pb.line_to(tx - head_len * (angle + head_angle).cos(), ty - head_len * (angle + head_angle).sin());

                if let Some(path) = pb.finish() {
                    let mut paint = Paint::default();
                    paint.set_color(color);
                    paint.anti_alias = true;
                    let mut stroke = Stroke::default();
                    stroke.width = stroke_width;
                    pixmap.stroke_path(&path, &paint, &stroke, transform, None);
                }
            }
            Object::Text(_) => {
                let content = state["content"].as_str().unwrap_or("").to_string();
                let x = state["x"].as_f64().unwrap_or(0.0) as f32;
                let y = state["y"].as_f64().unwrap_or(0.0) as f32;
                let font_size = state["font_size"].as_f64().unwrap_or(24.0) as f32;
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;
                let color = state["color"].as_str().unwrap_or("#FFFFFF").to_string();
                let font_id = state["font_id"].as_str().map(|s| s.to_string());

                let props = TextProps {
                    content,
                    x,
                    y,
                    font_size,
                    opacity,
                    color,
                    font_id,
                    z_index: 0,
                };

                let font = if let Some(fid) = &props.font_id {
                    self.text_engine.get_font(fid)
                } else {
                    self.text_engine.fonts().values().next()
                };

                if let Some(font) = font {
                    let mut x_cursor = props.x;
                    let color = parse_color(&props.color, opacity);

                    for c in props.content.chars() {
                        let (metrics, alpha) = font.rasterize(c, props.font_size);
                        let mut mask = Pixmap::new(metrics.width as u32, metrics.height as u32).unwrap();
                        
                        // Convert alpha mask to RGBA pixmap for tiny-skia
                        for (i, &a) in alpha.iter().enumerate() {
                            let r = color.red() as f32;
                            let g = color.green() as f32;
                            let b = color.blue() as f32;
                            let final_a = (a as f32 / 255.0) * color.alpha();
                            
                            mask.pixels_mut()[i] = Color::from_rgba(r, g, b, final_a).unwrap().premultiply().to_color_u8();
                        }

                        let glyph_transform = transform.pre_translate(
                            x_cursor + metrics.xmin as f32,
                            props.y - metrics.height as f32 - metrics.ymin as f32
                        );

                        pixmap.draw_pixmap(
                            0, 0,
                            mask.as_ref(),
                            &PixmapPaint::default(),
                            glyph_transform,
                            None
                        );

                        x_cursor += metrics.advance_width;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}

impl Renderer for SkiaRenderer {
    fn render_frame(
        &mut self,
        objects: &HashMap<String, Object>,
        states: &HashMap<String, Value>,
        width: u32,
        height: u32,
    ) -> Result<Vec<u8>, RendererError> {
        let mut pixmap = Pixmap::new(width, height).ok_or_else(|| {
            RendererError::Failed("Failed to create pixmap".to_string())
        })?;

        pixmap.fill(Color::from_rgba8(15, 15, 26, 255));

        let roots = self.get_root_objects(objects);
        for id in roots {
            self.draw_node(&mut pixmap, &id, objects, states, Transform::identity())?;
        }

        Ok(pixmap.data().to_vec())
    }

    fn load_font(&mut self, id: &str, data: &[u8]) -> Result<(), RendererError> {
        self.text_engine.load_font(id.to_string(), data).map_err(RendererError::Failed)
    }
}

fn parse_color(hex: &str, opacity: f32) -> Color {
    let hex = hex.trim_start_matches('#');
    if hex.len() == 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(255);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(255);
        Color::from_rgba8(r, g, b, (opacity * 255.0) as u8)
    } else if hex.len() == 8 {
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(255);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(255);
        let a = u8::from_str_radix(&hex[6..8], 16).unwrap_or(255);
        Color::from_rgba8(r, g, b, (a as f32 * opacity) as u8)
    } else {
        Color::WHITE
    }
}
