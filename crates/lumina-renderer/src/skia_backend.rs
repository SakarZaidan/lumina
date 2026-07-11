#![allow(clippy::field_reassign_with_default)]

use crate::{Renderer, RendererError};
use image::AnimationDecoder;
use lumina_schema::{CameraState, Object};
use lumina_text::TextEngine;
use serde_json::Value;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Cursor;
use tiny_skia::*;

/// A decoded raster asset held in the renderer. Static images store a single
/// premultiplied pixmap; animated GIFs store every frame plus its delay so the
/// correct frame can be selected from the current timeline position.
enum DecodedAsset {
    Static(Pixmap),
    Animated {
        frames: Vec<Pixmap>,
        delays_ms: Vec<u32>,
        total_ms: u32,
    },
}

struct AxesContext {
    origin_screen_x: f32,
    origin_screen_y: f32,
    scale: f32,
    x_min: f32,
    x_max: f32,
    y_min: f32,
    y_max: f32,
}

impl AxesContext {
    fn to_screen(&self, mx: f32, my: f32) -> (f32, f32) {
        (
            self.origin_screen_x + mx * self.scale,
            self.origin_screen_y - my * self.scale,
        )
    }
}

pub struct SkiaRenderer {
    text_engine: TextEngine,
    images: HashMap<String, DecodedAsset>,
    svgs: HashMap<String, resvg::usvg::Tree>,
    svg_cache: RefCell<HashMap<(String, u32, u32), Pixmap>>,
    current_time: f32,
}

impl SkiaRenderer {
    pub fn new() -> Self {
        Self {
            text_engine: TextEngine::new(),
            images: HashMap::new(),
            svgs: HashMap::new(),
            svg_cache: RefCell::new(HashMap::new()),
            current_time: 0.0,
        }
    }

    /// Pick the pixmap to draw for an asset id. For animated GIFs this selects
    /// the frame whose cumulative delay window contains the current timeline
    /// position (looping over the total duration).
    fn select_pixmap(&self, asset_id: &str) -> Option<&Pixmap> {
        match self.images.get(asset_id)? {
            DecodedAsset::Static(pm) => Some(pm),
            DecodedAsset::Animated {
                frames,
                delays_ms,
                total_ms,
            } => {
                if *total_ms == 0 {
                    return frames.first();
                }
                let t_ms = (self.current_time.max(0.0) * 1000.0) as u32 % *total_ms;
                let mut acc = 0u32;
                for (i, d) in delays_ms.iter().enumerate() {
                    acc += (*d).max(1);
                    if t_ms < acc {
                        return frames.get(i);
                    }
                }
                frames.last()
            }
        }
    }

    /// Rasterize an SVG asset to a pixmap at the requested size, caching the
    /// result by (id, width, height) since rasterization is expensive.
    fn rasterize_svg(
        &self,
        asset_id: &str,
        want_w: Option<f32>,
        want_h: Option<f32>,
    ) -> Option<Pixmap> {
        let tree = self.svgs.get(asset_id)?;
        let size = tree.size();
        let tw = want_w.unwrap_or_else(|| size.width()).round().max(1.0) as u32;
        let th = want_h.unwrap_or_else(|| size.height()).round().max(1.0) as u32;
        let key = (asset_id.to_string(), tw, th);
        if let Some(pm) = self.svg_cache.borrow().get(&key) {
            return Some(pm.clone());
        }
        let mut pm = Pixmap::new(tw, th)?;
        let sx = tw as f32 / size.width();
        let sy = th as f32 / size.height();
        resvg::render(tree, Transform::from_scale(sx, sy), &mut pm.as_mut());
        self.svg_cache.borrow_mut().insert(key, pm.clone());
        Some(pm)
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
            Object::MathML(p) => p.z_index,
            Object::Particles(p) => p.z_index,
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

    fn resolve_axes_context(
        &self,
        axes_id: &str,
        states: &HashMap<String, Value>,
    ) -> Option<AxesContext> {
        let s = states.get(axes_id)?;
        let x = s["x"].as_f64()? as f32;
        let y = s["y"].as_f64()? as f32;
        let x_range = s["x_range"].as_array()?;
        let y_range = s["y_range"].as_array()?;
        let x_min = x_range.first()?.as_f64()? as f32;
        let x_max = x_range.get(1)?.as_f64()? as f32;
        let y_min = y_range.first()?.as_f64()? as f32;
        let y_max = y_range.get(1)?.as_f64()? as f32;
        let scale = s["scale"].as_f64().unwrap_or(40.0) as f32;
        Some(AxesContext {
            origin_screen_x: x + (0.0 - x_min) * scale,
            origin_screen_y: y - (0.0 - y_min) * scale,
            scale,
            x_min,
            x_max,
            y_min,
            y_max,
        })
    }

    /// Rasterize a text run with per-character font fallback, horizontal
    /// alignment and letter-spacing, under the current transform.
    #[allow(clippy::too_many_arguments)]
    fn draw_text(
        &self,
        pixmap: &mut Pixmap,
        content: &str,
        x: f32,
        y: f32,
        font_size: f32,
        color_str: &str,
        font_id: Option<&str>,
        align: &str,
        letter_spacing: f32,
        opacity: f32,
        transform: Transform,
    ) {
        if content.is_empty() {
            return;
        }
        let color = parse_color(color_str, opacity);
        let start_x = match align {
            "center" => {
                x - self
                    .text_engine
                    .measure_width(content, font_size, font_id, letter_spacing)
                    / 2.0
            }
            "right" => {
                x - self
                    .text_engine
                    .measure_width(content, font_size, font_id, letter_spacing)
            }
            _ => x,
        };

        let mut x_cursor = start_x;
        for c in content.chars() {
            let font = match self.text_engine.font_for_char(c, font_id) {
                Some(f) => f,
                None => continue,
            };
            let (metrics, alpha) = font.rasterize(c, font_size);

            // Skip zero-size glyphs (spaces, control chars).
            if metrics.width == 0 || metrics.height == 0 {
                x_cursor += metrics.advance_width + letter_spacing;
                continue;
            }

            if let Some(mut mask) = Pixmap::new(metrics.width as u32, metrics.height as u32) {
                for (i, &a) in alpha.iter().enumerate() {
                    if i >= mask.pixels().len() {
                        break;
                    }
                    let final_a = (a as f32 / 255.0) * color.alpha();
                    mask.pixels_mut()[i] =
                        Color::from_rgba(color.red(), color.green(), color.blue(), final_a)
                            .unwrap_or(Color::WHITE)
                            .premultiply()
                            .to_color_u8();
                }

                // fontdue ymin: signed distance of glyph bottom from baseline.
                // top of glyph in screen coords = baseline - height - ymin
                let glyph_top = y - metrics.height as f32 - metrics.ymin as f32;
                let glyph_transform =
                    transform.pre_translate(x_cursor + metrics.xmin as f32, glyph_top);
                pixmap.draw_pixmap(
                    0,
                    0,
                    mask.as_ref(),
                    &PixmapPaint::default(),
                    glyph_transform,
                    None,
                );
            }

            x_cursor += metrics.advance_width + letter_spacing;
        }
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
        let state = states
            .get(id)
            .ok_or_else(|| RendererError::Failed(format!("No state for object '{}'", id)))?;

        match obj {
            Object::Group(props) => {
                let x = state["x"].as_f64().unwrap_or(0.0) as f32;
                let y = state["y"].as_f64().unwrap_or(0.0) as f32;
                let scale = state["scale"].as_f64().unwrap_or(1.0) as f32;
                // rotation is stored in degrees (user-facing unit)
                let rotation_deg = state["rotation"].as_f64().unwrap_or(0.0) as f32;

                let mut transform = parent_transform;
                transform = transform.pre_translate(x, y);
                if scale != 1.0 {
                    transform = transform.pre_scale(scale, scale);
                }
                if rotation_deg != 0.0 {
                    transform = transform.pre_rotate(rotation_deg);
                }

                // Sort children by z_index before drawing
                let mut children: Vec<(String, i32)> = props
                    .children
                    .iter()
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
                self.draw_leaf_object(pixmap, obj, state, parent_transform, states)?;
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
        states: &HashMap<String, Value>,
    ) -> Result<(), RendererError> {
        match obj {
            Object::Circle(_) => {
                let cx = state["cx"].as_f64().unwrap_or(0.0) as f32;
                let cy = state["cy"].as_f64().unwrap_or(0.0) as f32;
                let radius = state["radius"].as_f64().unwrap_or(0.0) as f32;
                if radius <= 0.0 {
                    return Ok(());
                }
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;

                let mut pb = PathBuilder::new();
                pb.push_circle(cx, cy, radius);
                if let Some(path) = pb.finish() {
                    let fill = parse_fill(&state["fill"], opacity)
                        .unwrap_or_else(|| FillStyle::Solid(parse_color("#FFFFFF", opacity)));
                    let stroke = parse_stroke(state, opacity);
                    let sw = state["stroke_width"].as_f64().unwrap_or(1.0) as f32;
                    let shadow = parse_shadow(state);
                    paint_shape(
                        pixmap,
                        &path,
                        transform,
                        Some(&fill),
                        stroke.as_ref(),
                        sw,
                        shadow.as_ref(),
                    );
                }
            }
            Object::Rectangle(_) => {
                let x = state["x"].as_f64().unwrap_or(0.0) as f32;
                let y = state["y"].as_f64().unwrap_or(0.0) as f32;
                let width = state["width"].as_f64().unwrap_or(0.0) as f32;
                let height = state["height"].as_f64().unwrap_or(0.0) as f32;
                if width <= 0.0 || height <= 0.0 {
                    return Ok(());
                }
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;

                let rx = state["rx"].as_f64().unwrap_or(0.0) as f32;
                let ry_raw = state["ry"].as_f64().unwrap_or(0.0) as f32;
                let ry = if ry_raw > 0.0 { ry_raw } else { rx };
                let fill = parse_fill(&state["fill"], opacity)
                    .unwrap_or_else(|| FillStyle::Solid(parse_color("#FFFFFF", opacity)));
                let stroke = parse_stroke(state, opacity);
                let sw = state["stroke_width"].as_f64().unwrap_or(1.0) as f32;
                let shadow = parse_shadow(state);

                if rx > 0.0 || shadow.is_some() {
                    // Rounded and/or shadowed rectangles go through the path renderer.
                    let path = if rx > 0.0 {
                        rounded_rect_path(x, y, width, height, rx, ry)
                    } else {
                        Rect::from_xywh(x, y, width, height).and_then(|rect| {
                            let mut pb = PathBuilder::new();
                            pb.push_rect(rect);
                            pb.finish()
                        })
                    };
                    if let Some(path) = path {
                        paint_shape(
                            pixmap,
                            &path,
                            transform,
                            Some(&fill),
                            stroke.as_ref(),
                            sw,
                            shadow.as_ref(),
                        );
                    }
                } else if let Some(rect) = Rect::from_xywh(x, y, width, height) {
                    // Fast path: axis-aligned fill (solid or gradient), no allocation.
                    let mut paint = Paint::default();
                    paint.anti_alias = true;
                    apply_fill(&mut paint, &fill, rect);
                    pixmap.fill_rect(rect, &paint, transform, None);

                    if let Some(s) = &stroke {
                        let mut pb = PathBuilder::new();
                        pb.push_rect(rect);
                        if let Some(path) = pb.finish() {
                            let mut spaint = Paint::default();
                            spaint.anti_alias = true;
                            apply_fill(&mut spaint, s, rect);
                            let mut st = Stroke::default();
                            st.width = sw;
                            pixmap.stroke_path(&path, &spaint, &st, transform, None);
                        }
                    }
                }
            }
            Object::Polygon(_) => {
                let points = state["points"].as_array().ok_or_else(|| {
                    RendererError::Failed(
                        "Polygon 'points' property is missing or not an array".into(),
                    )
                })?;
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;

                let mut pb = PathBuilder::new();
                for (i, p) in points.iter().enumerate() {
                    let arr = p.as_array().ok_or_else(|| {
                        RendererError::Failed(format!("Polygon point {} is not an array", i))
                    })?;
                    if arr.len() < 2 {
                        return Err(RendererError::Failed(format!(
                            "Polygon point {} has fewer than 2 coordinates",
                            i
                        )));
                    }
                    let x = arr[0].as_f64().unwrap_or(0.0) as f32;
                    let y = arr[1].as_f64().unwrap_or(0.0) as f32;
                    if i == 0 {
                        pb.move_to(x, y);
                    } else {
                        pb.line_to(x, y);
                    }
                }
                pb.close();

                if let Some(path) = pb.finish() {
                    let fill = parse_fill(&state["fill"], opacity)
                        .unwrap_or_else(|| FillStyle::Solid(parse_color("#FFFFFF", opacity)));
                    let stroke = parse_stroke(state, opacity);
                    let sw = state["stroke_width"].as_f64().unwrap_or(1.0) as f32;
                    let shadow = parse_shadow(state);
                    paint_shape(
                        pixmap,
                        &path,
                        transform,
                        Some(&fill),
                        stroke.as_ref(),
                        sw,
                        shadow.as_ref(),
                    );
                }
            }
            Object::Path(_) => {
                let d = state["d"].as_str().unwrap_or("");
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;

                if let Some(path) = parse_svg_path(d) {
                    let fill = parse_fill(&state["fill"], opacity);
                    let stroke = parse_stroke(state, opacity);
                    let sw = state["stroke_width"].as_f64().unwrap_or(1.0) as f32;
                    let shadow = parse_shadow(state);
                    paint_shape(
                        pixmap,
                        &path,
                        transform,
                        fill.as_ref(),
                        stroke.as_ref(),
                        sw,
                        shadow.as_ref(),
                    );
                }
            }
            Object::Line(_) => {
                let x1 = state["x1"].as_f64().unwrap_or(0.0) as f32;
                let y1 = state["y1"].as_f64().unwrap_or(0.0) as f32;
                let x2 = state["x2"].as_f64().unwrap_or(0.0) as f32;
                let y2 = state["y2"].as_f64().unwrap_or(0.0) as f32;
                let stroke_width = state["stroke_width"].as_f64().unwrap_or(1.0) as f32;
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;
                let stroke_color =
                    parse_color(state["stroke"].as_str().unwrap_or("#FFFFFF"), opacity);
                let draw_fraction = state["draw_fraction"].as_f64().map(|f| f as f32);

                let mut pb = PathBuilder::new();
                pb.move_to(x1, y1);
                pb.line_to(x2, y2);
                if let Some(path) = pb.finish() {
                    let mut paint = Paint::default();
                    paint.set_color(stroke_color);
                    paint.anti_alias = true;
                    let mut stroke = Stroke::default();
                    stroke.width = stroke_width;
                    if let Some(frac) = draw_fraction {
                        let frac = frac.clamp(0.0, 1.0);
                        let dx = x2 - x1;
                        let dy = y2 - y1;
                        let length = (dx * dx + dy * dy).sqrt().max(0.001);
                        stroke.dash = StrokeDash::new(vec![length * frac, length * 2.0], 0.0);
                    }
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
                    return Err(RendererError::Failed(
                        "Arrow 'from'/'to' arrays must have 2 elements".into(),
                    ));
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
                let content = state["content"].as_str().unwrap_or("");
                if content.is_empty() {
                    return Ok(());
                }
                let x = state["x"].as_f64().unwrap_or(0.0) as f32;
                let y = state["y"].as_f64().unwrap_or(0.0) as f32;
                let font_size = state["font_size"].as_f64().unwrap_or(24.0) as f32;
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;
                let color_str = state["color"].as_str().unwrap_or("#FFFFFF");
                let font_id = state["font_id"].as_str();
                let align = state["align"].as_str().unwrap_or("left");
                let letter_spacing = state["letter_spacing"].as_f64().unwrap_or(0.0) as f32;
                self.draw_text(
                    pixmap,
                    content,
                    x,
                    y,
                    font_size,
                    color_str,
                    font_id,
                    align,
                    letter_spacing,
                    opacity,
                    transform,
                );
            }
            Object::LaTeX(_) => {
                let expression = state["expression"].as_str().unwrap_or("");
                let x = state["x"].as_f64().unwrap_or(0.0) as f32;
                let y = state["y"].as_f64().unwrap_or(0.0) as f32;
                let font_size = state["font_size"].as_f64().unwrap_or(24.0) as f32;
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;
                let color_str = state["color"].as_str().unwrap_or("#FFFFFF");
                let font_id = state["font_id"].as_str();
                let align = state["align"].as_str().unwrap_or("left");
                let letter_spacing = state["letter_spacing"].as_f64().unwrap_or(0.0) as f32;
                let draw_fraction = state["draw_fraction"].as_f64().map(|f| f as f32);

                // Convert LaTeX/math notation to Unicode and optionally clip to
                // the first N characters for write-on animation.
                let mut rendered = latex_to_unicode(expression);
                if let Some(frac) = draw_fraction {
                    let frac = frac.clamp(0.0, 1.0);
                    let char_count = rendered.chars().count();
                    let visible = (char_count as f32 * frac).floor() as usize;
                    rendered = rendered.chars().take(visible).collect();
                }
                self.draw_text(
                    pixmap,
                    &rendered,
                    x,
                    y,
                    font_size,
                    color_str,
                    font_id,
                    align,
                    letter_spacing,
                    opacity,
                    transform,
                );
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
                        return Err(RendererError::Failed(
                            "BezierCurve point must have 2 elements".into(),
                        ));
                    }
                    Ok((
                        arr[0].as_f64().unwrap_or(0.0) as f32,
                        arr[1].as_f64().unwrap_or(0.0) as f32,
                    ))
                };

                let (x0, y0) = get_pt(p0)?;
                let (x1, y1) = get_pt(p1)?;
                let (x2, y2) = get_pt(p2)?;
                let (x3, y3) = get_pt(p3)?;
                let stroke_width = state["stroke_width"].as_f64().unwrap_or(1.0) as f32;
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;
                let stroke_color =
                    parse_color(state["stroke"].as_str().unwrap_or("#FFFFFF"), opacity);
                let draw_fraction = state["draw_fraction"].as_f64().map(|f| f as f32);

                let mut pb = PathBuilder::new();
                if let Some(frac) = draw_fraction {
                    // De Casteljau subdivision: clip cubic at parameter t=frac
                    let t = frac.clamp(0.0, 1.0);
                    let lerp = |a: f32, b: f32| a + (b - a) * t;
                    let ax = lerp(x0, x1);
                    let ay = lerp(y0, y1);
                    let bx = lerp(x1, x2);
                    let by = lerp(y1, y2);
                    let cx_ = lerp(x2, x3);
                    let cy_ = lerp(y2, y3);
                    let dx = lerp(ax, bx);
                    let dy = lerp(ay, by);
                    let ex = lerp(bx, cx_);
                    let ey = lerp(by, cy_);
                    let fx = lerp(dx, ex);
                    let fy = lerp(dy, ey);
                    pb.move_to(x0, y0);
                    pb.cubic_to(ax, ay, dx, dy, fx, fy);
                } else {
                    pb.move_to(x0, y0);
                    pb.cubic_to(x1, y1, x2, y2, x3, y3);
                }
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
                if range == 0.0 || step <= 0.0 {
                    return Ok(());
                }

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
                let x_range_arr = state["x_range"].as_array();
                let y_range_arr = state["y_range"].as_array();
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;
                let color = parse_color(state["color"].as_str().unwrap_or("#FFFFFF"), opacity);
                let scale = state["scale"].as_f64().unwrap_or(40.0) as f32;
                let x_step = state["x_step"].as_f64().unwrap_or(1.0) as f32;
                let y_step = state["y_step"].as_f64().unwrap_or(1.0) as f32;
                let draw_grid = state["grid"].as_bool().unwrap_or(false);

                let x_min = x_range_arr
                    .and_then(|r| r.first())
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0) as f32;
                let x_max = x_range_arr
                    .and_then(|r| r.get(1))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(10.0) as f32;
                let y_min = y_range_arr
                    .and_then(|r| r.first())
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0) as f32;
                let y_max = y_range_arr
                    .and_then(|r| r.get(1))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(10.0) as f32;

                // Screen position of math origin (0, 0)
                let ox = x + (0.0 - x_min) * scale;
                let oy = y - (0.0 - y_min) * scale;

                let mut paint = Paint::default();
                paint.set_color(color);
                paint.anti_alias = true;
                let mut stroke = Stroke::default();
                stroke.width = 2.0;

                // X axis (full range)
                let mut pb = PathBuilder::new();
                pb.move_to(ox + x_min * scale, oy);
                pb.line_to(ox + x_max * scale, oy);
                if let Some(path) = pb.finish() {
                    pixmap.stroke_path(&path, &paint, &stroke, transform, None);
                }
                // Y axis (full range)
                let mut pb = PathBuilder::new();
                pb.move_to(ox, oy - y_min * scale);
                pb.line_to(ox, oy - y_max * scale);
                if let Some(path) = pb.finish() {
                    pixmap.stroke_path(&path, &paint, &stroke, transform, None);
                }

                // Grid + ticks
                let grid_color =
                    parse_color(state["color"].as_str().unwrap_or("#FFFFFF"), opacity * 0.2);
                let mut grid_paint = Paint::default();
                grid_paint.set_color(grid_color);
                grid_paint.anti_alias = true;
                let mut grid_stroke = Stroke::default();
                grid_stroke.width = 1.0;
                let tick_h = 5.0_f32;

                // X ticks and vertical grid lines
                let x_count = ((x_max - x_min) / x_step).ceil() as i32;
                for i in 0..=x_count {
                    let tx = x_min + i as f32 * x_step;
                    if tx > x_max + 1e-4 {
                        break;
                    }
                    let px = ox + tx * scale;
                    let mut pb = PathBuilder::new();
                    pb.move_to(px, oy - tick_h);
                    pb.line_to(px, oy + tick_h);
                    if let Some(path) = pb.finish() {
                        pixmap.stroke_path(&path, &paint, &stroke, transform, None);
                    }
                    if draw_grid && (tx - 0.0).abs() > 1e-4 {
                        let mut pb = PathBuilder::new();
                        pb.move_to(px, oy - y_min * scale);
                        pb.line_to(px, oy - y_max * scale);
                        if let Some(path) = pb.finish() {
                            pixmap.stroke_path(&path, &grid_paint, &grid_stroke, transform, None);
                        }
                    }
                }

                // Y ticks and horizontal grid lines
                let y_count = ((y_max - y_min) / y_step).ceil() as i32;
                for i in 0..=y_count {
                    let ty = y_min + i as f32 * y_step;
                    if ty > y_max + 1e-4 {
                        break;
                    }
                    let py = oy - ty * scale;
                    let mut pb = PathBuilder::new();
                    pb.move_to(ox - tick_h, py);
                    pb.line_to(ox + tick_h, py);
                    if let Some(path) = pb.finish() {
                        pixmap.stroke_path(&path, &paint, &stroke, transform, None);
                    }
                    if draw_grid && (ty - 0.0).abs() > 1e-4 {
                        let mut pb = PathBuilder::new();
                        pb.move_to(ox + x_min * scale, py);
                        pb.line_to(ox + x_max * scale, py);
                        if let Some(path) = pb.finish() {
                            pixmap.stroke_path(&path, &grid_paint, &grid_stroke, transform, None);
                        }
                    }
                }
            }
            Object::Plot(props) => {
                let axes_id = state["axes_id"].as_str().unwrap_or(&props.axes_id);
                let function_str = state["function_str"]
                    .as_str()
                    .unwrap_or(&props.function_str);
                let stroke_width = state["stroke_width"].as_f64().unwrap_or(2.0) as f32;
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;
                let color = parse_color(state["color"].as_str().unwrap_or("#FFFFFF"), opacity);
                let total_samples = state["sample_count"].as_u64().unwrap_or(200) as usize;
                let draw_fraction = state["draw_fraction"].as_f64().map(|f| f as f32);
                let samples = if let Some(frac) = draw_fraction {
                    ((total_samples as f32 * frac.clamp(0.0, 1.0)) as usize).max(1)
                } else {
                    total_samples
                };

                let Some(ctx) = self.resolve_axes_context(axes_id, states) else {
                    return Ok(());
                };

                // Normalize bare math function names to evalexpr's math:: namespace
                // Only if not already namespaced (avoids double-prefixing)
                let normalized;
                let function_str: &str = if function_str.contains("math::") {
                    function_str
                } else {
                    normalized = function_str
                        .replace("sin(", "math::sin(")
                        .replace("cos(", "math::cos(")
                        .replace("tan(", "math::tan(")
                        .replace("sqrt(", "math::sqrt(")
                        .replace("abs(", "math::abs(")
                        .replace("exp(", "math::exp(")
                        .replace("ln(", "math::ln(");
                    &normalized
                };

                // When draw_fraction is set, clamp x_max to show only that portion of the domain
                let x_end = if let Some(frac) = draw_fraction {
                    ctx.x_min + (ctx.x_max - ctx.x_min) * frac.clamp(0.0, 1.0)
                } else {
                    ctx.x_max
                };

                let mut pb = PathBuilder::new();
                let mut started = false;
                for i in 0..=samples {
                    let mx = ctx.x_min + (i as f32 / samples as f32) * (x_end - ctx.x_min);
                    let eval_ctx = evalexpr::context_map! { "x" => mx as f64 };
                    let my = match eval_ctx
                        .and_then(|c| evalexpr::eval_number_with_context(function_str, &c))
                    {
                        Ok(v) => v as f32,
                        Err(_) => {
                            started = false;
                            continue;
                        }
                    };
                    let y_margin = (ctx.y_max - ctx.y_min).abs();
                    if !my.is_finite() || my < ctx.y_min - y_margin || my > ctx.y_max + y_margin {
                        started = false;
                        continue;
                    }
                    let (sx, sy) = ctx.to_screen(mx, my);
                    if !started {
                        pb.move_to(sx, sy);
                        started = true;
                    } else {
                        pb.line_to(sx, sy);
                    }
                }
                if let Some(path) = pb.finish() {
                    let mut paint = Paint::default();
                    paint.set_color(color);
                    paint.anti_alias = true;
                    let mut stroke = Stroke::default();
                    stroke.width = stroke_width;
                    stroke.line_cap = LineCap::Round;
                    stroke.line_join = LineJoin::Round;
                    pixmap.stroke_path(&path, &paint, &stroke, transform, None);
                }
            }
            Object::Image(_) => {
                let asset_id = state["asset_id"].as_str().unwrap_or("");
                if asset_id.is_empty() {
                    return Ok(());
                }
                let x = state["x"].as_f64().unwrap_or(0.0) as f32;
                let y = state["y"].as_f64().unwrap_or(0.0) as f32;
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;
                let rotation = state["rotation"].as_f64().unwrap_or(0.0) as f32;
                let want_w = state["width"].as_f64().map(|v| v as f32);
                let want_h = state["height"].as_f64().map(|v| v as f32);
                if let Some(src) = self.select_pixmap(asset_id) {
                    composite_image(
                        pixmap, src, x, y, want_w, want_h, rotation, opacity, transform,
                    );
                }
            }
            Object::SVG(_) => {
                let asset_id = state["asset_id"].as_str().unwrap_or("");
                if asset_id.is_empty() {
                    return Ok(());
                }
                let x = state["x"].as_f64().unwrap_or(0.0) as f32;
                let y = state["y"].as_f64().unwrap_or(0.0) as f32;
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;
                let rotation = state["rotation"].as_f64().unwrap_or(0.0) as f32;
                let want_w = state["width"].as_f64().map(|v| v as f32);
                let want_h = state["height"].as_f64().map(|v| v as f32);
                // SVG is rasterized at the requested size, so it is composited 1:1.
                if let Some(src) = self.rasterize_svg(asset_id, want_w, want_h) {
                    composite_image(pixmap, &src, x, y, None, None, rotation, opacity, transform);
                }
            }
            Object::MathML(_) => {
                let markup = state["markup"].as_str().unwrap_or("");
                if markup.is_empty() {
                    return Ok(());
                }
                let x = state["x"].as_f64().unwrap_or(0.0) as f32;
                let y = state["y"].as_f64().unwrap_or(0.0) as f32;
                let font_size = state["font_size"].as_f64().unwrap_or(24.0) as f32;
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;
                let color_str = state["color"].as_str().unwrap_or("#FFFFFF");
                let font_id = state["font_id"].as_str();
                let align = state["align"].as_str().unwrap_or("left");
                let letter_spacing = state["letter_spacing"].as_f64().unwrap_or(0.0) as f32;
                let rendered = mathml_to_unicode(markup);
                self.draw_text(
                    pixmap,
                    &rendered,
                    x,
                    y,
                    font_size,
                    color_str,
                    font_id,
                    align,
                    letter_spacing,
                    opacity,
                    transform,
                );
            }
            Object::Particles(_) => {
                let count = state["count"].as_u64().unwrap_or(0) as u32;
                if count == 0 {
                    return Ok(());
                }
                let ex = state["emitter_x"].as_f64().unwrap_or(0.0) as f32;
                let ey = state["emitter_y"].as_f64().unwrap_or(0.0) as f32;
                let lifetime = state["lifetime"].as_f64().unwrap_or(2.0) as f32;
                let speed = state["speed"].as_f64().unwrap_or(120.0) as f32;
                let spread = state["spread"].as_f64().unwrap_or(360.0) as f32;
                let size = state["size"].as_f64().unwrap_or(3.0) as f32;
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;
                let color_hex = state["color"].as_str().unwrap_or("#FFFFFF");
                draw_particles(
                    pixmap,
                    count,
                    ex,
                    ey,
                    lifetime,
                    speed,
                    spread,
                    size,
                    color_hex,
                    opacity,
                    self.current_time,
                    transform,
                );
            }
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
        camera: Option<&CameraState>,
    ) -> Result<Vec<u8>, RendererError> {
        let mut pixmap = Pixmap::new(width, height).ok_or_else(|| {
            RendererError::Failed(format!("Failed to create {}x{} pixmap", width, height))
        })?;

        pixmap.fill(parse_color(background, 1.0));

        let root_transform = if let Some(cam) = camera {
            let cx = width as f32 / 2.0;
            let cy = height as f32 / 2.0;
            Transform::from_translate(cx + cam.x, cy + cam.y)
                .pre_concat(Transform::from_scale(cam.zoom, cam.zoom))
                .pre_concat(Transform::from_translate(-cx, -cy))
        } else {
            Transform::identity()
        };

        let roots = self.get_root_objects_sorted(objects);
        for id in roots {
            self.draw_node(&mut pixmap, &id, objects, states, root_transform)?;
        }

        Ok(pixmap.data().to_vec())
    }

    fn load_font(&mut self, id: &str, data: &[u8]) -> Result<(), RendererError> {
        self.text_engine
            .load_font(id.to_string(), data)
            .map_err(RendererError::Failed)
    }

    fn load_image(&mut self, id: &str, data: &[u8]) -> Result<(), RendererError> {
        if is_svg(data) {
            let opt = resvg::usvg::Options::default();
            let tree = resvg::usvg::Tree::from_data(data, &opt)
                .map_err(|e| RendererError::Failed(format!("SVG parse failed for '{id}': {e}")))?;
            self.svgs.insert(id.to_string(), tree);
            return Ok(());
        }

        // Animated GIF: decode every frame and its delay.
        if image::guess_format(data).ok() == Some(image::ImageFormat::Gif) {
            let decoder = image::codecs::gif::GifDecoder::new(Cursor::new(data.to_vec()))
                .map_err(|e| RendererError::Failed(format!("GIF decode failed for '{id}': {e}")))?;
            let frames = decoder.into_frames().collect_frames().map_err(|e| {
                RendererError::Failed(format!("GIF frame decode failed for '{id}': {e}"))
            })?;
            if frames.len() > 1 {
                let mut pms = Vec::with_capacity(frames.len());
                let mut delays = Vec::with_capacity(frames.len());
                let mut total = 0u32;
                for f in frames {
                    let (num, den) = f.delay().numer_denom_ms();
                    let ms = if den == 0 { num } else { num / den.max(1) }.max(1);
                    let pm = rgba_to_pixmap(&f.into_buffer()).ok_or_else(|| {
                        RendererError::Failed(format!("GIF frame too large for '{id}'"))
                    })?;
                    pms.push(pm);
                    delays.push(ms);
                    total += ms;
                }
                self.images.insert(
                    id.to_string(),
                    DecodedAsset::Animated {
                        frames: pms,
                        delays_ms: delays,
                        total_ms: total,
                    },
                );
                return Ok(());
            }
            if let Some(f) = frames.into_iter().next() {
                let pm = rgba_to_pixmap(&f.into_buffer())
                    .ok_or_else(|| RendererError::Failed(format!("GIF too large for '{id}'")))?;
                self.images.insert(id.to_string(), DecodedAsset::Static(pm));
                return Ok(());
            }
        }

        // Static raster (PNG/JPEG/WebP/...).
        let img = image::load_from_memory(data)
            .map_err(|e| RendererError::Failed(format!("Image decode failed for '{id}': {e}")))?
            .to_rgba8();
        let pm = rgba_to_pixmap(&img)
            .ok_or_else(|| RendererError::Failed(format!("Image too large for '{id}'")))?;
        self.images.insert(id.to_string(), DecodedAsset::Static(pm));
        Ok(())
    }

    fn set_time(&mut self, time: f32) {
        self.current_time = time;
    }
}

/// Convert a straight-alpha RGBA8 image into a premultiplied tiny-skia pixmap.
fn rgba_to_pixmap(img: &image::RgbaImage) -> Option<Pixmap> {
    let (w, h) = img.dimensions();
    let mut pm = Pixmap::new(w, h)?;
    let dst = pm.pixels_mut();
    for (i, px) in img.pixels().enumerate() {
        let [r, g, b, a] = px.0;
        dst[i] = ColorU8::from_rgba(r, g, b, a).premultiply();
    }
    Some(pm)
}

/// Composite a source pixmap onto the destination honoring position, optional
/// resize (width/height), rotation (degrees, about the image center) and
/// opacity, all under the current parent (camera/group) transform.
#[allow(clippy::too_many_arguments)]
fn composite_image(
    dst: &mut Pixmap,
    src: &Pixmap,
    x: f32,
    y: f32,
    want_w: Option<f32>,
    want_h: Option<f32>,
    rotation: f32,
    opacity: f32,
    transform: Transform,
) {
    let sw = src.width() as f32;
    let sh = src.height() as f32;
    if sw <= 0.0 || sh <= 0.0 {
        return;
    }
    let scale_x = want_w.map(|w| w / sw).unwrap_or(1.0);
    let scale_y = want_h.map(|h| h / sh).unwrap_or(1.0);
    let dw = sw * scale_x;
    let dh = sh * scale_y;

    let mut local = Transform::from_translate(x, y);
    if rotation != 0.0 {
        local = local.pre_concat(Transform::from_rotate_at(rotation, dw / 2.0, dh / 2.0));
    }
    local = local.pre_concat(Transform::from_scale(scale_x, scale_y));
    let final_t = transform.pre_concat(local);

    let paint = PixmapPaint {
        opacity: opacity.clamp(0.0, 1.0),
        blend_mode: BlendMode::SourceOver,
        quality: FilterQuality::Bilinear,
    };
    dst.draw_pixmap(0, 0, src.as_ref(), &paint, final_t, None);
}

/// Heuristic: does this byte slice look like an SVG document?
fn is_svg(data: &[u8]) -> bool {
    let head = &data[..data.len().min(512)];
    let s = String::from_utf8_lossy(head);
    let trimmed = s.trim_start();
    trimmed.starts_with("<?xml") || trimmed.starts_with("<svg") || s.contains("<svg")
}

// Convert common LaTeX/math notation to Unicode for plain-text rendering.
pub(crate) fn latex_to_unicode(expr: &str) -> String {
    let mut s = expr.to_string();

    // Spacing commands → collapse to a single (or no) space.
    s = s
        .replace(r"\,", " ")
        .replace(r"\;", " ")
        .replace(r"\:", " ")
        .replace(r"\!", "")
        .replace(r"\quad", "  ")
        .replace(r"\qquad", "    ");

    // Greek letters
    s = s
        .replace(r"\alpha", "α")
        .replace(r"\beta", "β")
        .replace(r"\gamma", "γ")
        .replace(r"\delta", "δ")
        .replace(r"\epsilon", "ε")
        .replace(r"\zeta", "ζ")
        .replace(r"\eta", "η")
        .replace(r"\theta", "θ")
        .replace(r"\iota", "ι")
        .replace(r"\kappa", "κ")
        .replace(r"\lambda", "λ")
        .replace(r"\mu", "μ")
        .replace(r"\nu", "ν")
        .replace(r"\xi", "ξ")
        .replace(r"\pi", "π")
        .replace(r"\rho", "ρ")
        .replace(r"\sigma", "σ")
        .replace(r"\tau", "τ")
        .replace(r"\phi", "φ")
        .replace(r"\chi", "χ")
        .replace(r"\psi", "ψ")
        .replace(r"\omega", "ω");

    // Operators and symbols
    s = s
        .replace(r"\times", "×")
        .replace(r"\div", "÷")
        .replace(r"\pm", "±")
        .replace(r"\leq", "≤")
        .replace(r"\geq", "≥")
        .replace(r"\neq", "≠")
        .replace(r"\approx", "≈")
        .replace(r"\infty", "∞")
        .replace(r"\sum", "Σ")
        .replace(r"\prod", "Π")
        .replace(r"\int", "∫")
        .replace(r"\sqrt", "√")
        .replace(r"\cdot", "·")
        .replace(r"\circ", "∘")
        .replace(r"\in", "∈")
        .replace(r"\cup", "∪")
        .replace(r"\cap", "∩")
        .replace(r"\subset", "⊂")
        .replace(r"\rightarrow", "→")
        .replace(r"\leftarrow", "←")
        .replace(r"\Rightarrow", "⇒")
        .replace(r"\to", "→")
        .replace(r"\nabla", "∇")
        .replace(r"\partial", "∂")
        .replace(r"\cdots", "⋯")
        .replace(r"\ldots", "…")
        .replace(r"\angle", "∠")
        .replace(r"\vec", "")
        .replace(r"\left", "")
        .replace(r"\right", "");

    // Trig / named functions (LaTeX style)
    s = s
        .replace(r"\sin", "sin")
        .replace(r"\cos", "cos")
        .replace(r"\tan", "tan")
        .replace(r"\log", "log")
        .replace(r"\ln", "ln")
        .replace(r"\exp", "exp")
        .replace(r"\lim", "lim")
        .replace(r"\max", "max")
        .replace(r"\min", "min");

    // Fractions: \frac{a}{b} → a/b (brace-balanced, nestable).
    s = replace_frac(&s);

    // Super/subscripts: ^{...}, _{...}, bare ^N / _N.
    s = replace_scripts(&s);

    // Safety net: strip any remaining `\command` token (while braces still
    // delimit its argument) so unhandled commands never leak as literal text.
    s = strip_leftover_commands(&s);

    // Remove remaining LaTeX braces.
    s = s.replace(['{', '}'], "");

    s
}

/// Remove any remaining backslash command (`\` followed by ASCII letters), and
/// any lone backslash, leaving surrounding text intact.
fn strip_leftover_commands(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' {
            i += 1;
            while i < chars.len() && chars[i].is_ascii_alphabetic() {
                i += 1;
            }
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

/// Superscript Unicode for a character, if one exists.
fn superscript_of(c: char) -> Option<char> {
    Some(match c {
        '0' => '⁰',
        '1' => '¹',
        '2' => '²',
        '3' => '³',
        '4' => '⁴',
        '5' => '⁵',
        '6' => '⁶',
        '7' => '⁷',
        '8' => '⁸',
        '9' => '⁹',
        '+' => '⁺',
        '-' => '⁻',
        '=' => '⁼',
        '(' => '⁽',
        ')' => '⁾',
        'a' => 'ᵃ',
        'b' => 'ᵇ',
        'c' => 'ᶜ',
        'd' => 'ᵈ',
        'e' => 'ᵉ',
        'f' => 'ᶠ',
        'g' => 'ᵍ',
        'h' => 'ʰ',
        'i' => 'ⁱ',
        'j' => 'ʲ',
        'k' => 'ᵏ',
        'l' => 'ˡ',
        'm' => 'ᵐ',
        'n' => 'ⁿ',
        'o' => 'ᵒ',
        'p' => 'ᵖ',
        'r' => 'ʳ',
        's' => 'ˢ',
        't' => 'ᵗ',
        'u' => 'ᵘ',
        'v' => 'ᵛ',
        'w' => 'ʷ',
        'x' => 'ˣ',
        'y' => 'ʸ',
        'z' => 'ᶻ',
        _ => return None,
    })
}

/// Subscript Unicode for a character, if one exists.
fn subscript_of(c: char) -> Option<char> {
    Some(match c {
        '0' => '₀',
        '1' => '₁',
        '2' => '₂',
        '3' => '₃',
        '4' => '₄',
        '5' => '₅',
        '6' => '₆',
        '7' => '₇',
        '8' => '₈',
        '9' => '₉',
        '+' => '₊',
        '-' => '₋',
        '=' => '₌',
        '(' => '₍',
        ')' => '₎',
        'a' => 'ₐ',
        'e' => 'ₑ',
        'h' => 'ₕ',
        'i' => 'ᵢ',
        'j' => 'ⱼ',
        'k' => 'ₖ',
        'l' => 'ₗ',
        'm' => 'ₘ',
        'n' => 'ₙ',
        'o' => 'ₒ',
        'p' => 'ₚ',
        'r' => 'ᵣ',
        's' => 'ₛ',
        't' => 'ₜ',
        'u' => 'ᵤ',
        'v' => 'ᵥ',
        'x' => 'ₓ',
        _ => return None,
    })
}

/// Replace every `\frac{num}{den}` with `num/den`, honoring nested braces.
fn replace_frac(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i..].starts_with(&['\\', 'f', 'r', 'a', 'c']) {
            let after = i + 5;
            if let Some((num, j)) = read_brace_group(&chars, after) {
                if let Some((den, k)) = read_brace_group(&chars, j) {
                    // Recurse so nested fractions resolve too.
                    let num_s = replace_frac(&num.iter().collect::<String>());
                    let den_s = replace_frac(&den.iter().collect::<String>());
                    let wrap = |t: &str| {
                        if t.chars().count() > 1 && t.contains(['+', '-', ' ', '/']) {
                            format!("({t})")
                        } else {
                            t.to_string()
                        }
                    };
                    out.push_str(&wrap(&num_s));
                    out.push('/');
                    out.push_str(&wrap(&den_s));
                    i = k;
                    continue;
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// If `chars[start]` is `{`, return the balanced group contents and the index
/// just past the closing `}`.
fn read_brace_group(chars: &[char], start: usize) -> Option<(Vec<char>, usize)> {
    if start >= chars.len() || chars[start] != '{' {
        return None;
    }
    let mut depth = 0;
    let mut content = Vec::new();
    let mut i = start;
    while i < chars.len() {
        match chars[i] {
            '{' => {
                if depth > 0 {
                    content.push('{');
                }
                depth += 1;
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some((content, i + 1));
                }
                content.push('}');
            }
            c => content.push(c),
        }
        i += 1;
    }
    None
}

/// Convert `^{...}` / `_{...}` and bare `^c` / `_c` runs to Unicode super/subscripts.
/// Characters without a Unicode form keep the `^`/`_` marker so meaning isn't lost.
fn replace_scripts(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        let marker = chars[i];
        if marker == '^' || marker == '_' {
            let map = if marker == '^' {
                superscript_of
            } else {
                subscript_of
            };
            i += 1;
            // Collect the script content: a brace group or a single char.
            let content: Vec<char> = if i < chars.len() && chars[i] == '{' {
                match read_brace_group(&chars, i) {
                    Some((c, j)) => {
                        i = j;
                        c
                    }
                    None => vec![],
                }
            } else if i < chars.len() {
                let c = chars[i];
                i += 1;
                vec![c]
            } else {
                vec![]
            };
            // Map each char; if any has no script form, fall back to marker+raw.
            if content.iter().all(|&c| map(c).is_some()) {
                for &c in &content {
                    out.push(map(c).unwrap());
                }
            } else {
                out.push(marker);
                out.extend(content.iter());
            }
        } else {
            out.push(marker);
            i += 1;
        }
    }
    out
}

/// Strip MathML tags and decode common entities into a plain Unicode string,
/// reusing the same text pipeline as LaTeX.
pub(crate) fn mathml_to_unicode(markup: &str) -> String {
    let mut out = String::with_capacity(markup.len());
    let mut in_tag = false;
    for c in markup.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out = out
        .replace("&times;", "×")
        .replace("&divide;", "÷")
        .replace("&pm;", "±")
        .replace("&pi;", "π")
        .replace("&theta;", "θ")
        .replace("&alpha;", "α")
        .replace("&beta;", "β")
        .replace("&gamma;", "γ")
        .replace("&infin;", "∞")
        .replace("&le;", "≤")
        .replace("&ge;", "≥")
        .replace("&ne;", "≠")
        .replace("&sum;", "Σ")
        .replace("&int;", "∫")
        .replace("&radic;", "√")
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&");
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Render a particle emitter analytically from the current time so output is
/// fully reproducible (no RNG state between frames).
#[allow(clippy::too_many_arguments)]
fn draw_particles(
    pixmap: &mut Pixmap,
    count: u32,
    ex: f32,
    ey: f32,
    lifetime: f32,
    speed: f32,
    spread: f32,
    size: f32,
    color_hex: &str,
    opacity: f32,
    time: f32,
    transform: Transform,
) {
    let base = parse_color(color_hex, 1.0);
    for dot in crate::raster::simulate_particles(
        count, ex, ey, lifetime, speed, spread, size, opacity, time,
    ) {
        let mut pb = PathBuilder::new();
        pb.push_circle(dot.x, dot.y, dot.r);
        if let Some(path) = pb.finish() {
            let mut paint = Paint::default();
            let c =
                Color::from_rgba(base.red(), base.green(), base.blue(), dot.alpha).unwrap_or(base);
            paint.set_color(c);
            paint.anti_alias = true;
            pixmap.fill_path(&path, &paint, FillRule::Winding, transform, None);
        }
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
                cur_x = x;
                cur_y = y;
                i += 3;
            }
            "m" => {
                let dx = parse_f32!(i + 1);
                let dy = parse_f32!(i + 2);
                pb.move_to(cur_x + dx, cur_y + dy);
                cur_x += dx;
                cur_y += dy;
                i += 3;
            }
            "L" => {
                let x = parse_f32!(i + 1);
                let y = parse_f32!(i + 2);
                pb.line_to(x, y);
                cur_x = x;
                cur_y = y;
                i += 3;
            }
            "l" => {
                let dx = parse_f32!(i + 1);
                let dy = parse_f32!(i + 2);
                pb.line_to(cur_x + dx, cur_y + dy);
                cur_x += dx;
                cur_y += dy;
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
                cur_x = x;
                cur_y = y;
                i += 7;
            }
            "c" => {
                let dx1 = parse_f32!(i + 1);
                let dy1 = parse_f32!(i + 2);
                let dx2 = parse_f32!(i + 3);
                let dy2 = parse_f32!(i + 4);
                let dx = parse_f32!(i + 5);
                let dy = parse_f32!(i + 6);
                pb.cubic_to(
                    cur_x + dx1,
                    cur_y + dy1,
                    cur_x + dx2,
                    cur_y + dy2,
                    cur_x + dx,
                    cur_y + dy,
                );
                cur_x += dx;
                cur_y += dy;
                i += 7;
            }
            "Z" | "z" => {
                pb.close();
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }

    pb.finish()
}

/// A resolved fill/stroke source ready to apply to a tiny-skia `Paint`.
enum FillStyle {
    Solid(Color),
    Linear {
        stops: Vec<(f32, Color)>,
        angle_deg: f32,
    },
    Radial {
        stops: Vec<(f32, Color)>,
        radius_frac: f32,
    },
}

/// Parse a JSON value (hex string or gradient object) into a `FillStyle`.
fn parse_fill(v: &Value, opacity: f32) -> Option<FillStyle> {
    match v {
        Value::String(s) => Some(FillStyle::Solid(parse_color(s, opacity))),
        Value::Object(map) => {
            let kind = map.get("type").and_then(|t| t.as_str()).unwrap_or("linear");
            let stops = parse_stops(map.get("stops"), opacity)?;
            match kind {
                "radial" => {
                    let radius_frac =
                        map.get("radius").and_then(|r| r.as_f64()).unwrap_or(0.5) as f32;
                    Some(FillStyle::Radial { stops, radius_frac })
                }
                _ => {
                    let angle_deg = map.get("angle").and_then(|a| a.as_f64()).unwrap_or(0.0) as f32;
                    Some(FillStyle::Linear { stops, angle_deg })
                }
            }
        }
        _ => None,
    }
}

/// Stroke is optional: absent or null means "no stroke".
fn parse_stroke(state: &Value, opacity: f32) -> Option<FillStyle> {
    match state.get("stroke") {
        Some(v) if !v.is_null() => parse_fill(v, opacity),
        _ => None,
    }
}

fn parse_stops(v: Option<&Value>, opacity: f32) -> Option<Vec<(f32, Color)>> {
    let arr = v?.as_array()?;
    let mut out = Vec::with_capacity(arr.len());
    for s in arr {
        let pair = s.as_array()?;
        let pos = pair.first()?.as_f64()? as f32;
        let hex = pair.get(1)?.as_str()?;
        out.push((pos.clamp(0.0, 1.0), parse_color(hex, opacity)));
    }
    if out.len() < 2 {
        return None;
    }
    Some(out)
}

/// Apply a `FillStyle` to a paint, deriving gradient geometry from the shape
/// bounding box.
fn apply_fill(paint: &mut Paint, fill: &FillStyle, bbox: Rect) {
    match fill {
        FillStyle::Solid(c) => {
            paint.set_color(*c);
        }
        FillStyle::Linear { stops, angle_deg } => {
            let gstops: Vec<GradientStop> = stops
                .iter()
                .map(|(p, c)| GradientStop::new(*p, *c))
                .collect();
            let cx = bbox.x() + bbox.width() / 2.0;
            let cy = bbox.y() + bbox.height() / 2.0;
            let r = bbox.width().max(bbox.height()) / 2.0;
            let rad = angle_deg.to_radians();
            let (dx, dy) = (rad.cos(), rad.sin());
            let start = Point::from_xy(cx - dx * r, cy - dy * r);
            let end = Point::from_xy(cx + dx * r, cy + dy * r);
            if let Some(shader) =
                LinearGradient::new(start, end, gstops, SpreadMode::Pad, Transform::identity())
            {
                paint.shader = shader;
            }
        }
        FillStyle::Radial { stops, radius_frac } => {
            let gstops: Vec<GradientStop> = stops
                .iter()
                .map(|(p, c)| GradientStop::new(*p, *c))
                .collect();
            let cx = bbox.x() + bbox.width() / 2.0;
            let cy = bbox.y() + bbox.height() / 2.0;
            let r = (bbox.width().max(bbox.height()) / 2.0) * radius_frac.max(0.01);
            let center = Point::from_xy(cx, cy);
            if let Some(shader) = RadialGradient::new(
                center,
                center,
                r.max(0.01),
                gstops,
                SpreadMode::Pad,
                Transform::identity(),
            ) {
                paint.shader = shader;
            }
        }
    }
}

/// A resolved drop-shadow specification.
struct ShadowSpec {
    color: Color,
    blur: f32,
    dx: f32,
    dy: f32,
    opacity: f32,
}

fn parse_shadow(state: &Value) -> Option<ShadowSpec> {
    let map = match state.get("shadow") {
        Some(Value::Object(m)) => m,
        _ => return None,
    };
    let color_hex = map
        .get("color")
        .and_then(|c| c.as_str())
        .unwrap_or("#000000");
    Some(ShadowSpec {
        color: parse_color(color_hex, 1.0),
        blur: map.get("blur").and_then(|b| b.as_f64()).unwrap_or(0.0) as f32,
        dx: map.get("dx").and_then(|d| d.as_f64()).unwrap_or(0.0) as f32,
        dy: map.get("dy").and_then(|d| d.as_f64()).unwrap_or(0.0) as f32,
        opacity: map.get("opacity").and_then(|o| o.as_f64()).unwrap_or(1.0) as f32,
    })
}

/// Render a blurred, offset silhouette of `path` beneath a shape.
fn draw_shadow(dst: &mut Pixmap, path: &Path, transform: Transform, shadow: &ShadowSpec) {
    let mut off = match Pixmap::new(dst.width(), dst.height()) {
        Some(p) => p,
        None => return,
    };
    let mut paint = Paint::default();
    paint.set_color(shadow.color);
    paint.anti_alias = true;
    let t = Transform::from_translate(shadow.dx, shadow.dy).pre_concat(transform);
    off.fill_path(path, &paint, FillRule::Winding, t, None);
    let r = shadow.blur.clamp(0.0, 50.0).round() as usize;
    box_blur(&mut off, r);
    let pp = PixmapPaint {
        opacity: shadow.opacity.clamp(0.0, 1.0),
        blend_mode: BlendMode::SourceOver,
        quality: FilterQuality::Nearest,
    };
    dst.draw_pixmap(0, 0, off.as_ref(), &pp, Transform::identity(), None);
}

/// Separable 3-pass box blur (≈ Gaussian) over premultiplied RGBA bytes.
fn box_blur(pm: &mut Pixmap, radius: usize) {
    if radius == 0 {
        return;
    }
    let w = pm.width() as usize;
    let h = pm.height() as usize;
    if w == 0 || h == 0 {
        return;
    }
    let data = pm.data_mut();
    let mut tmp = vec![0u8; data.len()];
    for _ in 0..3 {
        blur_pass_h(data, &mut tmp, w, h, radius);
        blur_pass_v(&tmp, data, w, h, radius);
    }
}

fn blur_pass_h(src: &[u8], dst: &mut [u8], w: usize, h: usize, r: usize) {
    let win = (2 * r + 1) as u32;
    for y in 0..h {
        let base = y * w;
        for c in 0..4 {
            let get = |x: usize| src[(base + x.min(w - 1)) * 4 + c] as u32;
            let mut sum: u32 = 0;
            for i in 0..=2 * r {
                sum += get(i.saturating_sub(r));
            }
            for x in 0..w {
                dst[(base + x) * 4 + c] = (sum / win) as u8;
                let add_idx = (x + r + 1).min(w - 1);
                let sub_idx = x.saturating_sub(r);
                sum += get(add_idx);
                sum -= get(sub_idx);
            }
        }
    }
}

fn blur_pass_v(src: &[u8], dst: &mut [u8], w: usize, h: usize, r: usize) {
    let win = (2 * r + 1) as u32;
    for x in 0..w {
        for c in 0..4 {
            let get = |y: usize| src[(y.min(h - 1) * w + x) * 4 + c] as u32;
            let mut sum: u32 = 0;
            for i in 0..=2 * r {
                sum += get(i.saturating_sub(r));
            }
            for y in 0..h {
                dst[(y * w + x) * 4 + c] = (sum / win) as u8;
                let add_idx = (y + r + 1).min(h - 1);
                let sub_idx = y.saturating_sub(r);
                sum += get(add_idx);
                sum -= get(sub_idx);
            }
        }
    }
}

/// Build a rounded-rectangle path using quadratic corner arcs.
fn rounded_rect_path(x: f32, y: f32, w: f32, h: f32, rx: f32, ry: f32) -> Option<Path> {
    let rx = rx.min(w / 2.0).max(0.0);
    let ry = ry.min(h / 2.0).max(0.0);
    let mut pb = PathBuilder::new();
    pb.move_to(x + rx, y);
    pb.line_to(x + w - rx, y);
    pb.quad_to(x + w, y, x + w, y + ry);
    pb.line_to(x + w, y + h - ry);
    pb.quad_to(x + w, y + h, x + w - rx, y + h);
    pb.line_to(x + rx, y + h);
    pb.quad_to(x, y + h, x, y + h - ry);
    pb.line_to(x, y + ry);
    pb.quad_to(x, y, x + rx, y);
    pb.close();
    pb.finish()
}

/// Paint a closed path: optional shadow, then fill, then stroke.
fn paint_shape(
    pixmap: &mut Pixmap,
    path: &Path,
    transform: Transform,
    fill: Option<&FillStyle>,
    stroke: Option<&FillStyle>,
    stroke_width: f32,
    shadow: Option<&ShadowSpec>,
) {
    let bbox = path.bounds();
    if let Some(sh) = shadow {
        draw_shadow(pixmap, path, transform, sh);
    }
    if let Some(f) = fill {
        let mut paint = Paint::default();
        paint.anti_alias = true;
        apply_fill(&mut paint, f, bbox);
        pixmap.fill_path(path, &paint, FillRule::Winding, transform, None);
    }
    if let Some(s) = stroke {
        let mut paint = Paint::default();
        paint.anti_alias = true;
        apply_fill(&mut paint, s, bbox);
        let mut st = Stroke::default();
        st.width = stroke_width;
        pixmap.stroke_path(path, &paint, &st, transform, None);
    }
}

pub(crate) fn parse_color(hex: &str, opacity: f32) -> Color {
    crate::common::color::to_tiny(crate::common::color::parse_rgba8(hex, opacity))
}
