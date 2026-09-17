#![allow(clippy::field_reassign_with_default)]

use crate::common::fill::{fill_spec, FillSpec};
pub use crate::common::notation::latex_to_unicode;
use crate::common::shadow::shadow_spec;
use crate::{Renderer, RendererError};
use image::AnimationDecoder;
use luminafx_schema::{CameraState, Object};
use luminafx_text::TextEngine;
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

/// CPU reference renderer over `tiny-skia`. Full feature coverage; the
/// GPU backend is held to this one's output by the parity suite.
pub struct SkiaRenderer {
    text_engine: TextEngine,
    images: HashMap<String, DecodedAsset>,
    svgs: HashMap<String, resvg::usvg::Tree>,
    svg_cache: RefCell<HashMap<(String, u32, u32), Pixmap>>,
    current_time: f32,
    /// The frame buffer, kept between renders.
    ///
    /// A 1080p `Pixmap` is 8.3 MB. Allocating and dropping one per frame
    /// measured **5.3 ms/frame**; reusing one measures **0.57 ms** — the
    /// allocator returns a block that size to the operating system, so every
    /// frame faults in fresh pages. On an ordinary scene that was roughly 87%
    /// of the total render time, and it applied to every scene regardless of
    /// content (see `planning/METRICS.md`).
    ///
    /// Reallocated only when the requested size changes, which for a render is
    /// once. Correctness is unaffected because the buffer is cleared to the
    /// background before anything is drawn — the same operation that always
    /// began a frame.
    frame: Option<Pixmap>,
}

impl SkiaRenderer {
    /// A renderer with no assets loaded.
    pub fn new() -> Self {
        Self {
            frame: None,
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

    fn resolve_axes_context(
        &self,
        axes_id: &str,
        objects: &HashMap<String, Object>,
    ) -> Option<AxesContext> {
        // A plot drawn against anything but an Axes draws nothing; validation
        // reports it as AXES_ID_IS_NOT_AXES.
        let Some(Object::Axes(axes)) = objects.get(axes_id) else {
            return None;
        };
        let [x_min, x_max] = axes.x_range;
        let [y_min, y_max] = axes.y_range;
        Some(AxesContext {
            origin_screen_x: axes.x + (0.0 - x_min) * axes.scale,
            origin_screen_y: axes.y - (0.0 - y_min) * axes.scale,
            scale: axes.scale,
            x_min,
            x_max,
            y_min,
            y_max,
        })
    }

    /// Rasterize a text run with per-character font fallback, horizontal
    /// alignment and letter-spacing, under the current transform.
    fn draw_text(
        &self,
        pixmap: &mut Pixmap,
        content: &str,
        style: &crate::common::text::TextStyle<'_>,
        transform: Transform,
    ) {
        let color = parse_color(style.color, style.opacity);
        let (x, y) = (style.x, style.y);
        let Some(layout) = crate::common::text::layout_run(
            &self.text_engine,
            content,
            style.font_size,
            style.font_id,
            style.align,
            style.letter_spacing,
        ) else {
            return;
        };

        // The only step that differs from the GPU backend: each glyph goes
        // straight into the frame under the live transform, rather than into a
        // standalone bitmap that is then drawn as an image.
        for placed in &layout.glyphs {
            // The sub-pixel remainder is baked into the mask; the transform
            // gets the whole-pixel part, which is all `draw_pixmap` would have
            // honoured anyway.
            let at = placed.place(x, y);
            let Some(mask) = crate::common::text::glyph_mask(&placed.glyph, color, at.fx, at.fy)
            else {
                continue;
            };
            pixmap.draw_pixmap(
                0,
                0,
                mask.as_ref(),
                &PixmapPaint::default(),
                transform.pre_translate(at.ix as f32, at.iy as f32),
                None,
            );
        }
    }

    /// Walk one node of the scene graph, recursing through groups.
    ///
    /// `depth` bounds the recursion. Scenes reaching a renderer have normally
    /// been validated (`GROUP_NESTING_TOO_DEEP`), but the renderer is a public
    /// API in its own right and a deep chain here would overflow the stack,
    /// which aborts the process rather than returning an error.
    fn draw_node(
        &self,
        pixmap: &mut Pixmap,
        id: &str,
        objects: &HashMap<String, Object>,
        parent_transform: Transform,
        depth: usize,
    ) -> Result<(), RendererError> {
        if depth > luminafx_core::validation::MAX_GROUP_DEPTH {
            return Err(RendererError::Failed(format!(
                "group nesting deeper than {} at '{id}'",
                luminafx_core::validation::MAX_GROUP_DEPTH
            )));
        }
        let obj = objects.get(id).ok_or_else(|| {
            RendererError::Failed(format!("Object '{id}' not found in scene graph"))
        })?;
        match obj {
            Object::Group(props) => {
                let transform = crate::common::scene::group_transform(
                    crate::common::scene::Mat2x3::from_tiny(parent_transform),
                    props,
                )
                .to_tiny();

                for child_id in crate::common::scene::sorted_children(&props.children, objects) {
                    self.draw_node(pixmap, child_id, objects, transform, depth + 1)?;
                }
            }
            _ => {
                self.draw_leaf_object(pixmap, obj, parent_transform, objects)?;
            }
        }
        Ok(())
    }

    fn draw_leaf_object(
        &self,
        pixmap: &mut Pixmap,
        obj: &Object,
        transform: Transform,
        objects: &HashMap<String, Object>,
    ) -> Result<(), RendererError> {
        match obj {
            Object::Circle(props) => {
                if props.radius <= 0.0 {
                    return Ok(());
                }
                let mut pb = PathBuilder::new();
                pb.push_circle(props.cx, props.cy, props.radius);
                if let Some(path) = pb.finish() {
                    let fill = fill_spec(&props.fill, props.opacity)
                        .unwrap_or_else(|| FillSpec::solid("#FFFFFF", props.opacity));
                    let stroke = props
                        .stroke
                        .as_ref()
                        .and_then(|paint| fill_spec(paint, props.opacity));
                    let shadow = props.shadow.as_ref().map(shadow_spec);
                    paint_shape(
                        pixmap,
                        &path,
                        transform,
                        Some(&fill),
                        stroke.as_ref(),
                        props.stroke_width,
                        shadow.as_ref(),
                    );
                }
            }
            Object::Rectangle(props) => {
                let (x, y, width, height) = (props.x, props.y, props.width, props.height);
                if width <= 0.0 || height <= 0.0 {
                    return Ok(());
                }
                let rx = props.rx;
                let ry = if props.ry > 0.0 { props.ry } else { rx };
                let fill = fill_spec(&props.fill, props.opacity)
                    .unwrap_or_else(|| FillSpec::solid("#FFFFFF", props.opacity));
                let stroke = props
                    .stroke
                    .as_ref()
                    .and_then(|paint| fill_spec(paint, props.opacity));
                let sw = props.stroke_width;
                let shadow = props.shadow.as_ref().map(shadow_spec);

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
            Object::Polygon(props) => {
                // `[f32; 2]` points: the missing-array and short-point errors
                // this branch used to raise cannot be expressed any more.
                let mut pb = PathBuilder::new();
                for (i, &[x, y]) in props.points.iter().enumerate() {
                    if i == 0 {
                        pb.move_to(x, y);
                    } else {
                        pb.line_to(x, y);
                    }
                }
                pb.close();

                if let Some(path) = pb.finish() {
                    let fill = fill_spec(&props.fill, props.opacity)
                        .unwrap_or_else(|| FillSpec::solid("#FFFFFF", props.opacity));
                    let stroke = props
                        .stroke
                        .as_ref()
                        .and_then(|paint| fill_spec(paint, props.opacity));
                    let shadow = props.shadow.as_ref().map(shadow_spec);
                    paint_shape(
                        pixmap,
                        &path,
                        transform,
                        Some(&fill),
                        stroke.as_ref(),
                        props.stroke_width,
                        shadow.as_ref(),
                    );
                }
            }
            Object::Path(props) => {
                if let Some(mut data) = crate::common::path::parse_svg_path(&props.d) {
                    // Trimmed by arc length, which is what `draw_fraction`
                    // means for every stroked object.
                    if let Some(frac) = props.draw_fraction {
                        data = crate::common::path::trim(&data, frac);
                    }
                    let Some(path) = crate::common::path::to_tiny_path(&data) else {
                        return Ok(());
                    };
                    let fill = fill_spec(&props.fill, props.opacity);
                    let stroke = props
                        .stroke
                        .as_ref()
                        .and_then(|paint| fill_spec(paint, props.opacity));
                    let shadow = props.shadow.as_ref().map(shadow_spec);
                    paint_shape(
                        pixmap,
                        &path,
                        transform,
                        fill.as_ref(),
                        stroke.as_ref(),
                        props.stroke_width,
                        shadow.as_ref(),
                    );
                }
            }
            Object::Line(props) => {
                let (x1, y1, x2, y2) = (props.x1, props.y1, props.x2, props.y2);
                let stroke_color = parse_color(&props.stroke, props.opacity);

                let mut pb = PathBuilder::new();
                pb.move_to(x1, y1);
                pb.line_to(x2, y2);
                if let Some(path) = pb.finish() {
                    let mut paint = Paint::default();
                    paint.set_color(stroke_color);
                    paint.anti_alias = true;
                    let mut stroke = Stroke::default();
                    stroke.width = props.stroke_width;
                    // `draw_fraction` also works by dashing, so it wins when
                    // both are present: a line being revealed should reveal,
                    // not reveal-and-dash. TD-19.
                    if let Some(frac) = props.draw_fraction {
                        let dx = x2 - x1;
                        let dy = y2 - y1;
                        let length = (dx * dx + dy * dy).sqrt().max(0.001);
                        stroke.dash = StrokeDash::new(
                            crate::common::stroke::draw_fraction_dash(frac, length),
                            0.0,
                        );
                    } else if let Some(pattern) =
                        crate::common::stroke::dash_pattern(props.dash.as_deref())
                    {
                        stroke.dash = StrokeDash::new(pattern, 0.0);
                    }
                    pixmap.stroke_path(&path, &paint, &stroke, transform, None);
                }
            }
            Object::Arrow(props) => {
                // `[f32; 2]` endpoints: the malformed-array errors this branch
                // raised (#53) cannot be expressed any more.
                let [fx, fy] = props.from;
                let [tx, ty] = props.to;
                let stroke_width = props.stroke_width;
                let color = parse_color(&props.color, props.opacity);

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
            Object::Text(_) | Object::LaTeX(_) | Object::MathML(_) => {
                let Some((text, style)) = crate::common::text::text_of(obj) else {
                    return Ok(());
                };
                if text.is_empty() {
                    return Ok(());
                }
                self.draw_text(pixmap, &text, &style, transform);
            }
            Object::BezierCurve(props) => {
                let [x0, y0] = props.p0;
                let [x1, y1] = props.p1;
                let [x2, y2] = props.p2;
                let [x3, y3] = props.p3;
                let stroke_width = props.stroke_width;
                let stroke_color = parse_color(&props.stroke, props.opacity);
                let draw_fraction = props.draw_fraction;

                // Trimmed by arc length, shared with Path. De Casteljau at
                // parameter `t` was exact but measured the wrong thing: a cubic
                // traversed at uniform `t` does not move at uniform speed, so
                // the reveal visibly accelerated and slowed along the curve
                // while `draw_fraction` climbed steadily.
                let curve =
                    crate::common::path::PathData::cubic((x0, y0), (x1, y1), (x2, y2), (x3, y3));
                let curve = match draw_fraction {
                    Some(frac) => crate::common::path::trim(&curve, frac),
                    None => curve,
                };
                if let Some(path) = crate::common::path::to_tiny_path(&curve) {
                    let mut paint = Paint::default();
                    paint.set_color(stroke_color);
                    paint.anti_alias = true;
                    let mut stroke = Stroke::default();
                    stroke.width = stroke_width;
                    pixmap.stroke_path(&path, &paint, &stroke, transform, None);
                }
            }
            Object::NumberLine(props) => {
                let (start, end, step) = (props.start, props.end, props.step);
                let (x, y) = (props.x, props.y);
                let length = props.length.unwrap_or(400.0);
                let color = parse_color(&props.color, props.opacity);
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
                for i in 0..crate::common::ticks::count(start, end, step) {
                    let t = crate::common::ticks::at(start, step, i);
                    let px = x + (t - start) / range * length;
                    let mut pb = PathBuilder::new();
                    pb.move_to(px, y - tick_h);
                    pb.line_to(px, y + tick_h);
                    if let Some(path) = pb.finish() {
                        pixmap.stroke_path(&path, &paint, &stroke, transform, None);
                    }
                }
            }
            Object::Axes(props) => {
                let (x, y) = (props.x, props.y);
                let opacity = props.opacity;
                let color = parse_color(&props.color, opacity);
                let (scale, x_step, y_step) = (props.scale, props.x_step, props.y_step);
                let draw_grid = props.grid;
                let [x_min, x_max] = props.x_range;
                let [y_min, y_max] = props.y_range;

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
                let grid_color = parse_color(&props.color, opacity * 0.2);
                let mut grid_paint = Paint::default();
                grid_paint.set_color(grid_color);
                grid_paint.anti_alias = true;
                let mut grid_stroke = Stroke::default();
                grid_stroke.width = 1.0;
                let tick_h = 5.0_f32;

                // X ticks and vertical grid lines
                let x_count = crate::common::ticks::count(x_min, x_max, x_step);
                for i in 0..x_count {
                    let tx = crate::common::ticks::at(x_min, x_step, i);
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
                let y_count = crate::common::ticks::count(y_min, y_max, y_step);
                for i in 0..y_count {
                    let ty = crate::common::ticks::at(y_min, y_step, i);
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
                let function_str = props.function_str.as_str();
                let stroke_width = props.stroke_width;
                let color = parse_color(&props.color, props.opacity);
                let samples = props.sample_count as usize;
                let draw_fraction = props.draw_fraction;

                let Some(ctx) = self.resolve_axes_context(&props.axes_id, objects) else {
                    return Ok(());
                };

                // draw_fraction reveals the curve by narrowing the domain, not
                // by sampling it more coarsely — the old code scaled the sample
                // count, so the curve visibly changed resolution as it drew.
                let x_end = if let Some(frac) = draw_fraction {
                    f64::from(ctx.x_min)
                        + (f64::from(ctx.x_max) - f64::from(ctx.x_min))
                            * f64::from(frac.clamp(0.0, 1.0))
                } else {
                    f64::from(ctx.x_max)
                };

                let segments = crate::common::plot::sample(
                    function_str,
                    f64::from(ctx.x_min),
                    x_end,
                    f64::from(ctx.y_min),
                    f64::from(ctx.y_max),
                    samples,
                );

                let mut pb = PathBuilder::new();
                for segment in &segments {
                    for (i, (mx, my)) in segment.iter().enumerate() {
                        let (sx, sy) = ctx.to_screen(*mx as f32, *my as f32);
                        if i == 0 {
                            pb.move_to(sx, sy);
                        } else {
                            pb.line_to(sx, sy);
                        }
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
            Object::Image(props) => {
                if props.asset_id.is_empty() {
                    return Ok(());
                }
                if let Some(src) = self.select_pixmap(&props.asset_id) {
                    composite_image(
                        pixmap,
                        src,
                        props.x,
                        props.y,
                        props.width,
                        props.height,
                        props.rotation,
                        props.opacity,
                        transform,
                    );
                }
            }
            Object::SVG(props) => {
                if props.asset_id.is_empty() {
                    return Ok(());
                }
                // SVG is rasterized at the requested size, so it is composited 1:1.
                if let Some(src) = self.rasterize_svg(&props.asset_id, props.width, props.height) {
                    composite_image(
                        pixmap,
                        &src,
                        props.x,
                        props.y,
                        None,
                        None,
                        props.rotation,
                        props.opacity,
                        transform,
                    );
                }
            }
            Object::Particles(props) => {
                if props.count == 0 {
                    return Ok(());
                }
                draw_particles(
                    pixmap,
                    props.count,
                    props.emitter_x,
                    props.emitter_y,
                    props.lifetime,
                    props.speed,
                    props.spread,
                    props.size,
                    &props.color,
                    props.opacity,
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
        width: u32,
        height: u32,
        background: &str,
        camera: Option<&CameraState>,
    ) -> Result<Vec<u8>, RendererError> {
        // Take the buffer out so `self` stays borrowable for `draw_node`.
        let mut pixmap = match self.frame.take() {
            Some(p) if p.width() == width && p.height() == height => p,
            _ => Pixmap::new(width, height).ok_or_else(|| {
                RendererError::Failed(format!("Failed to create {width}x{height} pixmap"))
            })?,
        };

        // Clears every pixel, so nothing survives from the previous frame.
        pixmap.fill(parse_color(background, 1.0));

        let root_transform =
            crate::common::scene::camera_transform(camera, width, height).to_tiny();

        for id in crate::common::scene::sorted_root_ids(objects) {
            if let Err(e) = self.draw_node(&mut pixmap, id, objects, root_transform, 0) {
                // Put the buffer back before returning, or the next frame pays
                // the allocation this exists to avoid.
                self.frame = Some(pixmap);
                return Err(e);
            }
        }

        let out = pixmap.data().to_vec();
        self.frame = Some(pixmap);
        Ok(out)
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

/// Apply a `FillSpec` to a paint, deriving gradient geometry from the shape
/// bounding box via the shared helpers.
fn apply_fill(paint: &mut Paint<'_>, fill: &crate::common::fill::FillSpec, bbox: Rect) {
    use crate::common::fill::FillSpec;
    let bb = (bbox.x(), bbox.y(), bbox.width(), bbox.height());
    match fill {
        FillSpec::Solid(c) => {
            paint.set_color(crate::common::color::to_tiny(*c));
        }
        FillSpec::Linear { stops, angle_deg } => {
            let gstops: Vec<GradientStop> = stops
                .iter()
                .map(|(p, c)| GradientStop::new(*p, crate::common::color::to_tiny(*c)))
                .collect();
            let (start, end) = crate::common::fill::linear_geometry(bb, *angle_deg);
            if let Some(shader) = LinearGradient::new(
                Point::from_xy(start.0, start.1),
                Point::from_xy(end.0, end.1),
                gstops,
                SpreadMode::Pad,
                Transform::identity(),
            ) {
                paint.shader = shader;
            }
        }
        FillSpec::Radial { stops, radius_frac } => {
            let gstops: Vec<GradientStop> = stops
                .iter()
                .map(|(p, c)| GradientStop::new(*p, crate::common::color::to_tiny(*c)))
                .collect();
            let (center, r) = crate::common::fill::radial_geometry(bb, *radius_frac);
            let center = Point::from_xy(center.0, center.1);
            // tiny-skia 0.12 two-circle form; (center, 0) → (center, r) is
            // the classic single-circle radial the schema describes.
            if let Some(shader) = RadialGradient::new(
                center,
                0.0,
                center,
                r,
                gstops,
                SpreadMode::Pad,
                Transform::identity(),
            ) {
                paint.shader = shader;
            }
        }
    }
}

/// Build a rounded-rectangle path via the shared quadratic-arc geometry.
fn rounded_rect_path(x: f32, y: f32, w: f32, h: f32, rx: f32, ry: f32) -> Option<Path> {
    crate::common::path::to_tiny_path(&crate::common::path::rounded_rect(x, y, w, h, rx, ry))
}

/// Paint a closed path: optional shadow, then fill, then stroke.
fn paint_shape(
    pixmap: &mut Pixmap,
    path: &Path,
    transform: Transform,
    fill: Option<&crate::common::fill::FillSpec>,
    stroke: Option<&crate::common::fill::FillSpec>,
    stroke_width: f32,
    shadow: Option<&crate::common::shadow::ShadowSpec>,
) {
    let bbox = path.bounds();
    if let Some(sh) = shadow {
        crate::common::shadow::draw_shadow(pixmap, path, transform, sh);
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
