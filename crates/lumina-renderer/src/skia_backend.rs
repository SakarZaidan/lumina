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

    fn z_index(obj: &Object) -> i32 {
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
            Object::Image(p) => p.z_index,
            Object::SVG(p) => p.z_index,
            Object::NumberLine(p) => p.z_index,
            Object::Axes(p) => p.z_index,
            Object::Plot(p) => p.z_index,
            Object::BezierCurve(p) => p.z_index,
        }
    }

    fn get_root_objects_sorted(&self, objects: &HashMap<String, Object>) -> Vec<String> {
        let mut child_ids = std::collections::HashSet::new();
        for obj in objects.values() {
            if let Object::Group(group) = obj {
                for child_id in &group.children {
                    child_ids.insert(child_id.clone());
                }
            }
        }

        let mut roots: Vec<(String, i32)> = objects
            .iter()
            .filter(|(id, _)| !child_ids.contains(*id))
            .map(|(id, obj)| (id.clone(), Self::z_index(obj)))
            .collect();

        roots.sort_by_key(|(_, z)| *z);
        roots.into_iter().map(|(id, _)| id).collect()
    }

    fn draw_node(
        &self,
        pixmap: &mut Pixmap,
        id: &str,
        objects: &HashMap<String, Object>,
        states: &HashMap<String, Value>,
        parent_transform: Transform,
    ) -> Result<(), RendererError> {
        let obj = objects.get(id).ok_or_else(|| {
            RendererError::Failed(format!("Object '{}' not found in scene graph", id))
        })?;
        let state = states.get(id).ok_or_else(|| {
            RendererError::Failed(format!("No state for object '{}'", id))
        })?;

        match obj {
            Object::Group(props) => {
                let x = state["x"].as_f64().unwrap_or(0.0) as f32;
                let y = state["y"].as_f64().unwrap_or(0.0) as f32;
                let scale = state["scale"].as_f64().unwrap_or(1.0) as f32;
                // rotation is stored in degrees (user-facing unit)
                let rotation_deg = state["rotation"].as_f64().unwrap_or(0.0) as f32;

                let mut transform = parent_transform;
                transform = transform.pre_translate(x, y);
                if scale != 1.0 { transform = transform.pre_scale(scale, scale); }
                if rotation_deg != 0.0 { transform = transform.pre_rotate(rotation_deg); }

                // Sort children by z_index before drawing
                let mut children: Vec<(String, i32)> = props.children.iter()
                    .map(|cid| {
                        let z = objects.get(cid).map(Self::z_index).unwrap_or(0);
                        (cid.clone(), z)
                    })
                    .collect();
                children.sort_by_key(|(_, z)| *z);

                for (child_id, _) in children {
                    self.draw_node(pixmap, &child_id, objects, states, transform)?;
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
                if radius <= 0.0 { return Ok(()); }
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;

                let mut pb = PathBuilder::new();
                pb.push_circle(cx, cy, radius);
                if let Some(path) = pb.finish() {
                    let fill_color = parse_color(state["fill"].as_str().unwrap_or("#FFFFFF"), opacity);
                    let mut paint = Paint::default();
                    paint.set_color(fill_color);
                    paint.anti_alias = true;
                    pixmap.fill_path(&path, &paint, FillRule::Winding, transform, None);

                    if let Some(stroke_hex) = state["stroke"].as_str() {
                        let sw = state["stroke_width"].as_f64().unwrap_or(1.0) as f32;
                        let stroke_color = parse_color(stroke_hex, opacity);
                        let mut spaint = Paint::default();
                        spaint.set_color(stroke_color);
                        spaint.anti_alias = true;
                        let mut stroke = Stroke::default();
                        stroke.width = sw;
                        pixmap.stroke_path(&path, &spaint, &stroke, transform, None);
                    }
                }
            }
            Object::Rectangle(_) => {
                let x = state["x"].as_f64().unwrap_or(0.0) as f32;
                let y = state["y"].as_f64().unwrap_or(0.0) as f32;
                let width = state["width"].as_f64().unwrap_or(0.0) as f32;
                let height = state["height"].as_f64().unwrap_or(0.0) as f32;
                if width <= 0.0 || height <= 0.0 { return Ok(()); }
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;

                if let Some(rect) = Rect::from_xywh(x, y, width, height) {
                    let fill_color = parse_color(state["fill"].as_str().unwrap_or("#FFFFFF"), opacity);
                    let mut paint = Paint::default();
                    paint.set_color(fill_color);
                    paint.anti_alias = true;
                    pixmap.fill_rect(rect, &paint, transform, None);

                    if let Some(stroke_hex) = state["stroke"].as_str() {
                        let sw = state["stroke_width"].as_f64().unwrap_or(1.0) as f32;
                        let stroke_color = parse_color(stroke_hex, opacity);
                        let mut pb = PathBuilder::new();
                        pb.push_rect(rect);
                        if let Some(path) = pb.finish() {
                            let mut spaint = Paint::default();
                            spaint.set_color(stroke_color);
                            spaint.anti_alias = true;
                            let mut stroke = Stroke::default();
                            stroke.width = sw;
                            pixmap.stroke_path(&path, &spaint, &stroke, transform, None);
                        }
                    }
                }
            }
            Object::Polygon(_) => {
                let points = state["points"].as_array().ok_or_else(|| {
                    RendererError::Failed("Polygon 'points' property is missing or not an array".into())
                })?;
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;
                let fill_color = parse_color(state["fill"].as_str().unwrap_or("#FFFFFF"), opacity);

                let mut pb = PathBuilder::new();
                for (i, p) in points.iter().enumerate() {
                    let arr = p.as_array().ok_or_else(|| {
                        RendererError::Failed(format!("Polygon point {} is not an array", i))
                    })?;
                    if arr.len() < 2 {
                        return Err(RendererError::Failed(format!("Polygon point {} has fewer than 2 coordinates", i)));
                    }
                    let x = arr[0].as_f64().unwrap_or(0.0) as f32;
                    let y = arr[1].as_f64().unwrap_or(0.0) as f32;
                    if i == 0 { pb.move_to(x, y); } else { pb.line_to(x, y); }
                }
                pb.close();

                if let Some(path) = pb.finish() {
                    let mut paint = Paint::default();
                    paint.set_color(fill_color);
                    paint.anti_alias = true;
                    pixmap.fill_path(&path, &paint, FillRule::Winding, transform, None);

                    if let Some(stroke_hex) = state["stroke"].as_str() {
                        let sw = state["stroke_width"].as_f64().unwrap_or(1.0) as f32;
                        let stroke_color = parse_color(stroke_hex, opacity);
                        let mut spaint = Paint::default();
                        spaint.set_color(stroke_color);
                        spaint.anti_alias = true;
                        let mut stroke = Stroke::default();
                        stroke.width = sw;
                        pixmap.stroke_path(&path, &spaint, &stroke, transform, None);
                    }
                }
            }
            Object::Path(_) => {
                let d = state["d"].as_str().unwrap_or("");
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;

                if let Some(path) = parse_svg_path(d) {
                    if let Some(fill_hex) = state["fill"].as_str() {
                        let fill_color = parse_color(fill_hex, opacity);
                        let mut paint = Paint::default();
                        paint.set_color(fill_color);
                        paint.anti_alias = true;
                        pixmap.fill_path(&path, &paint, FillRule::Winding, transform, None);
                    }
                    if let Some(stroke_hex) = state["stroke"].as_str() {
                        let sw = state["stroke_width"].as_f64().unwrap_or(1.0) as f32;
                        let stroke_color = parse_color(stroke_hex, opacity);
                        let mut spaint = Paint::default();
                        spaint.set_color(stroke_color);
                        spaint.anti_alias = true;
                        let mut stroke = Stroke::default();
                        stroke.width = sw;
                        pixmap.stroke_path(&path, &spaint, &stroke, transform, None);
                    }
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
                let from = state["from"].as_array().ok_or_else(|| {
                    RendererError::Failed("Arrow 'from' property is missing or not an array".into())
                })?;
                let to = state["to"].as_array().ok_or_else(|| {
                    RendererError::Failed("Arrow 'to' property is missing or not an array".into())
                })?;
                if from.len() < 2 || to.len() < 2 {
                    return Err(RendererError::Failed("Arrow 'from'/'to' arrays must have 2 elements".into()));
                }

                let fx = from[0].as_f64().unwrap_or(0.0) as f32;
                let fy = from[1].as_f64().unwrap_or(0.0) as f32;
                let tx = to[0].as_f64().unwrap_or(0.0) as f32;
                let ty = to[1].as_f64().unwrap_or(0.0) as f32;
                let stroke_width = state["stroke_width"].as_f64().unwrap_or(1.0) as f32;
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;
                let color = parse_color(state["color"].as_str().unwrap_or("#FFFFFF"), opacity);

                let mut pb = PathBuilder::new();
                pb.move_to(fx, fy);
                pb.line_to(tx, ty);

                let dx = tx - fx;
                let dy = ty - fy;
                let angle = dy.atan2(dx);
                // Scale arrowhead with stroke width
                let head_len = (stroke_width * 5.0).max(10.0);
                let head_angle = std::f32::consts::PI / 6.0;

                pb.move_to(tx, ty);
                pb.line_to(
                    tx - head_len * (angle - head_angle).cos(),
                    ty - head_len * (angle - head_angle).sin(),
                );
                pb.move_to(tx, ty);
                pb.line_to(
                    tx - head_len * (angle + head_angle).cos(),
                    ty - head_len * (angle + head_angle).sin(),
                );

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
                if content.is_empty() { return Ok(()); }

                let x = state["x"].as_f64().unwrap_or(0.0) as f32;
                let y = state["y"].as_f64().unwrap_or(0.0) as f32;
                let font_size = state["font_size"].as_f64().unwrap_or(24.0) as f32;
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;
                let color_str = state["color"].as_str().unwrap_or("#FFFFFF");
                let font_id = state["font_id"].as_str().map(|s| s.to_string());

                let props = TextProps {
                    content: content.clone(),
                    x,
                    y,
                    font_size,
                    opacity,
                    color: color_str.to_string(),
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

                        // Skip zero-size glyphs (spaces, control chars)
                        if metrics.width == 0 || metrics.height == 0 {
                            x_cursor += metrics.advance_width;
                            continue;
                        }

                        if let Some(mut mask) = Pixmap::new(metrics.width as u32, metrics.height as u32) {
                            for (i, &a) in alpha.iter().enumerate() {
                                if i >= mask.pixels().len() { break; }
                                let final_a = (a as f32 / 255.0) * color.alpha();
                                mask.pixels_mut()[i] = Color::from_rgba(
                                    color.red(),
                                    color.green(),
                                    color.blue(),
                                    final_a,
                                ).unwrap_or(Color::WHITE).premultiply().to_color_u8();
                            }

                            // Correct baseline: y is the baseline, glyph drawn above it
                            // fontdue ymin is negative for glyphs below baseline (descenders)
                            let glyph_top = props.y - metrics.height as f32 + metrics.ymin as f32;
                            let glyph_transform = transform.pre_translate(
                                x_cursor + metrics.xmin as f32,
                                glyph_top,
                            );

                            pixmap.draw_pixmap(
                                0, 0,
                                mask.as_ref(),
                                &PixmapPaint::default(),
                                glyph_transform,
                                None,
                            );
                        }

                        x_cursor += metrics.advance_width;
                    }
                }
            }
            Object::LaTeX(_) => {
                // MiTeX integration is pending — render expression as plain text fallback
                let expression = state["expression"].as_str().unwrap_or("");
                let x = state["x"].as_f64().unwrap_or(0.0) as f32;
                let y = state["y"].as_f64().unwrap_or(0.0) as f32;
                let font_size = state["font_size"].as_f64().unwrap_or(24.0) as f32;
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;
                let color_str = state["color"].as_str().unwrap_or("#FFFFFF");

                let fallback_state = serde_json::json!({
                    "content": expression,
                    "x": x,
                    "y": y,
                    "font_size": font_size,
                    "opacity": opacity,
                    "color": color_str,
                });
                let text_obj = Object::Text(lumina_schema::TextProps {
                    content: expression.to_string(),
                    x, y, font_size,
                    opacity,
                    color: color_str.to_string(),
                    font_id: None,
                    z_index: 0,
                });
                self.draw_leaf_object(pixmap, &text_obj, &fallback_state, transform)?;
            }
            Object::BezierCurve(_) => {
                let p0 = state["p0"].as_array().ok_or_else(|| {
                    RendererError::Failed("BezierCurve 'p0' is missing or not an array".into())
                })?;
                let p1 = state["p1"].as_array().ok_or_else(|| {
                    RendererError::Failed("BezierCurve 'p1' is missing or not an array".into())
                })?;
                let p2 = state["p2"].as_array().ok_or_else(|| {
                    RendererError::Failed("BezierCurve 'p2' is missing or not an array".into())
                })?;
                let p3 = state["p3"].as_array().ok_or_else(|| {
                    RendererError::Failed("BezierCurve 'p3' is missing or not an array".into())
                })?;
                let get_pt = |arr: &Vec<Value>| -> Result<(f32, f32), RendererError> {
                    if arr.len() < 2 {
                        return Err(RendererError::Failed("BezierCurve point must have 2 elements".into()));
                    }
                    Ok((arr[0].as_f64().unwrap_or(0.0) as f32, arr[1].as_f64().unwrap_or(0.0) as f32))
                };

                let (x0, y0) = get_pt(p0)?;
                let (x1, y1) = get_pt(p1)?;
                let (x2, y2) = get_pt(p2)?;
                let (x3, y3) = get_pt(p3)?;
                let stroke_width = state["stroke_width"].as_f64().unwrap_or(1.0) as f32;
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;
                let stroke_color = parse_color(state["stroke"].as_str().unwrap_or("#FFFFFF"), opacity);

                let mut pb = PathBuilder::new();
                pb.move_to(x0, y0);
                pb.cubic_to(x1, y1, x2, y2, x3, y3);
                if let Some(path) = pb.finish() {
                    let mut paint = Paint::default();
                    paint.set_color(stroke_color);
                    paint.anti_alias = true;
                    let mut stroke = Stroke::default();
                    stroke.width = stroke_width;
                    pixmap.stroke_path(&path, &paint, &stroke, transform, None);
                }
            }
            Object::NumberLine(_) => {
                let start = state["start"].as_f64().unwrap_or(0.0) as f32;
                let end = state["end"].as_f64().unwrap_or(10.0) as f32;
                let step = state["step"].as_f64().unwrap_or(1.0) as f32;
                let x = state["x"].as_f64().unwrap_or(0.0) as f32;
                let y = state["y"].as_f64().unwrap_or(0.0) as f32;
                let length = state["length"].as_f64().unwrap_or(400.0) as f32;
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;
                let color = parse_color(state["color"].as_str().unwrap_or("#FFFFFF"), opacity);
                let range = end - start;
                if range == 0.0 || step <= 0.0 { return Ok(()); }

                let mut paint = Paint::default();
                paint.set_color(color);
                paint.anti_alias = true;
                let mut stroke = Stroke::default();
                stroke.width = 2.0;

                // Main axis line
                let mut pb = PathBuilder::new();
                pb.move_to(x, y);
                pb.line_to(x + length, y);
                if let Some(path) = pb.finish() {
                    pixmap.stroke_path(&path, &paint, &stroke, transform, None);
                }

                // Tick marks
                let tick_h = 8.0;
                let mut t = start;
                while t <= end + 1e-4 {
                    let px = x + (t - start) / range * length;
                    let mut pb = PathBuilder::new();
                    pb.move_to(px, y - tick_h);
                    pb.line_to(px, y + tick_h);
                    if let Some(path) = pb.finish() {
                        pixmap.stroke_path(&path, &paint, &stroke, transform, None);
                    }
                    t += step;
                }
            }
            Object::Axes(_) => {
                let x = state["x"].as_f64().unwrap_or(0.0) as f32;
                let y = state["y"].as_f64().unwrap_or(0.0) as f32;
                let x_range = state["x_range"].as_array();
                let y_range = state["y_range"].as_array();
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;
                let color = parse_color(state["color"].as_str().unwrap_or("#FFFFFF"), opacity);

                let x_max = x_range.and_then(|r| r.get(1)).and_then(|v| v.as_f64()).unwrap_or(10.0) as f32;
                let y_max = y_range.and_then(|r| r.get(1)).and_then(|v| v.as_f64()).unwrap_or(10.0) as f32;
                let scale = 40.0_f32;

                let mut paint = Paint::default();
                paint.set_color(color);
                paint.anti_alias = true;
                let mut stroke = Stroke::default();
                stroke.width = 2.0;

                // X axis
                let mut pb = PathBuilder::new();
                pb.move_to(x, y);
                pb.line_to(x + x_max * scale, y);
                if let Some(path) = pb.finish() {
                    pixmap.stroke_path(&path, &paint, &stroke, transform, None);
                }
                // Y axis
                let mut pb = PathBuilder::new();
                pb.move_to(x, y);
                pb.line_to(x, y - y_max * scale);
                if let Some(path) = pb.finish() {
                    pixmap.stroke_path(&path, &paint, &stroke, transform, None);
                }
            }
            // Image and SVG rendering require asset loading — not yet wired up
            Object::Image(_) | Object::SVG(_) | Object::Plot(_) => {}
            Object::Group(_) => {} // handled in draw_node
        }
        Ok(())
    }
}

impl Default for SkiaRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer for SkiaRenderer {
    fn render_frame(
        &mut self,
        objects: &HashMap<String, Object>,
        states: &HashMap<String, Value>,
        width: u32,
        height: u32,
        background: &str,
    ) -> Result<Vec<u8>, RendererError> {
        let mut pixmap = Pixmap::new(width, height).ok_or_else(|| {
            RendererError::Failed(format!("Failed to create {}x{} pixmap", width, height))
        })?;

        pixmap.fill(parse_color(background, 1.0));

        let roots = self.get_root_objects_sorted(objects);
        for id in roots {
            self.draw_node(&mut pixmap, &id, objects, states, Transform::identity())?;
        }

        Ok(pixmap.data().to_vec())
    }

    fn load_font(&mut self, id: &str, data: &[u8]) -> Result<(), RendererError> {
        self.text_engine.load_font(id.to_string(), data).map_err(RendererError::Failed)
    }
}

// Parse SVG path data string into a tiny-skia Path.
// Supports: M/m (move), L/l (line), H/h (horizontal), V/v (vertical),
//           C/c (cubic bezier), Z/z (close).
fn parse_svg_path(d: &str) -> Option<Path> {
    let mut pb = PathBuilder::new();

    // Normalize: insert spaces around command letters, replace commas with spaces
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
    let mut cur_x = 0.0_f32;
    let mut cur_y = 0.0_f32;

    macro_rules! parse_f32 {
        ($idx:expr) => {
            tokens.get($idx).and_then(|s| s.parse::<f32>().ok())?
        };
    }

    while i < tokens.len() {
        match tokens[i] {
            "M" => {
                let x = parse_f32!(i + 1);
                let y = parse_f32!(i + 2);
                pb.move_to(x, y);
                cur_x = x; cur_y = y;
                i += 3;
            }
            "m" => {
                let dx = parse_f32!(i + 1);
                let dy = parse_f32!(i + 2);
                pb.move_to(cur_x + dx, cur_y + dy);
                cur_x += dx; cur_y += dy;
                i += 3;
            }
            "L" => {
                let x = parse_f32!(i + 1);
                let y = parse_f32!(i + 2);
                pb.line_to(x, y);
                cur_x = x; cur_y = y;
                i += 3;
            }
            "l" => {
                let dx = parse_f32!(i + 1);
                let dy = parse_f32!(i + 2);
                pb.line_to(cur_x + dx, cur_y + dy);
                cur_x += dx; cur_y += dy;
                i += 3;
            }
            "H" => {
                let x = parse_f32!(i + 1);
                pb.line_to(x, cur_y);
                cur_x = x;
                i += 2;
            }
            "h" => {
                let dx = parse_f32!(i + 1);
                pb.line_to(cur_x + dx, cur_y);
                cur_x += dx;
                i += 2;
            }
            "V" => {
                let y = parse_f32!(i + 1);
                pb.line_to(cur_x, y);
                cur_y = y;
                i += 2;
            }
            "v" => {
                let dy = parse_f32!(i + 1);
                pb.line_to(cur_x, cur_y + dy);
                cur_y += dy;
                i += 2;
            }
            "C" => {
                let x1 = parse_f32!(i + 1);
                let y1 = parse_f32!(i + 2);
                let x2 = parse_f32!(i + 3);
                let y2 = parse_f32!(i + 4);
                let x = parse_f32!(i + 5);
                let y = parse_f32!(i + 6);
                pb.cubic_to(x1, y1, x2, y2, x, y);
                cur_x = x; cur_y = y;
                i += 7;
            }
            "c" => {
                let dx1 = parse_f32!(i + 1);
                let dy1 = parse_f32!(i + 2);
                let dx2 = parse_f32!(i + 3);
                let dy2 = parse_f32!(i + 4);
                let dx = parse_f32!(i + 5);
                let dy = parse_f32!(i + 6);
                pb.cubic_to(cur_x + dx1, cur_y + dy1, cur_x + dx2, cur_y + dy2, cur_x + dx, cur_y + dy);
                cur_x += dx; cur_y += dy;
                i += 7;
            }
            "Z" | "z" => {
                pb.close();
                i += 1;
            }
            _ => { i += 1; }
        }
    }

    pb.finish()
}

fn parse_color(hex: &str, opacity: f32) -> Color {
    let hex = hex.trim_start_matches('#');
    match hex.len() {
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255);
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(255);
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(255);
            Color::from_rgba8(r, g, b, (opacity * 255.0) as u8)
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255);
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(255);
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(255);
            let a = u8::from_str_radix(&hex[6..8], 16).unwrap_or(255);
            Color::from_rgba8(r, g, b, (a as f32 * opacity) as u8)
        }
        3 => {
            // Short form: #RGB → #RRGGBB
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).unwrap_or(255);
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).unwrap_or(255);
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).unwrap_or(255);
            Color::from_rgba8(r, g, b, (opacity * 255.0) as u8)
        }
        _ => Color::WHITE,
    }
}
