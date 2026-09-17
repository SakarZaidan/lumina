use crate::common::fill::{fill_spec, FillSpec};
use crate::common::shadow::shadow_spec;
use crate::raster;
use crate::{Renderer, RendererError};
use luminafx_schema::{CameraState, Object};
use luminafx_text::TextEngine;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Cursor;
use std::num::NonZeroUsize;

use image::AnimationDecoder;
// Use vello's re-exported wgpu (v0.20) — workspace wgpu is v22, types are incompatible
use vello::wgpu;
use vello::{
    kurbo::{Affine, BezPath, Cap, Circle, Join, Line, Rect, Stroke as KurboStroke, Vec2},
    peniko::{
        BlendMode, Blob, Brush, Color, ColorStop, Compose, Fill, Format, Gradient, Image, Mix,
    },
    AaConfig, AaSupport, RenderParams, RendererOptions, Scene,
};

/// A decoded asset held by the GPU backend. Mirrors the Skia backend's asset
/// model but stores straight-alpha `peniko::Image`s ready for `draw_image`.
enum VelloAsset {
    Static(Image),
    Animated {
        frames: Vec<Image>,
        delays_ms: Vec<u32>,
        total_ms: u32,
    },
    Svg(Box<resvg::usvg::Tree>),
}

/// GPU renderer over `vello`/`wgpu`, running headless with the CPU
/// fallback adapter forced so it works in CI and containers. Feature
/// parity with [`crate::skia_backend::SkiaRenderer`] is enforced by the
/// pixel-diff parity suite.
pub struct VelloRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: vello::Renderer,
    text_engine: TextEngine,
    images: HashMap<String, VelloAsset>,
    svg_cache: RefCell<HashMap<(String, u32, u32), Image>>,
    current_time: f32,
}

/// The parts of a scene walk that do not change as it descends.
///
/// Bundled so `draw_node` carries only the arguments that vary — node,
/// transform, depth. The alternative was an eight-parameter recursive
/// function where three parameters held the same value at every level.
struct WalkCtx<'a> {
    objects: &'a HashMap<String, Object>,
    canvas: (u32, u32),
}

impl VelloRenderer {
    /// Create a new `VelloRenderer`, blocking the current thread during GPU init.
    pub fn new() -> Result<Self, RendererError> {
        pollster::block_on(Self::new_async())
    }

    async fn new_async() -> Result<Self, RendererError> {
        // Escape hatch for environments where adapter probing is not merely
        // unavailable but unsafe: the Windows DX12-WARP fallback aborts the
        // process instead of reporting failure, so a graceful `Err` is never
        // reached (TD-20). Set `LUMINA_DISABLE_VELLO=1` to skip probing.
        if std::env::var("LUMINA_DISABLE_VELLO").as_deref() == Ok("1") {
            return Err(RendererError::Failed(
                "vello backend disabled by LUMINA_DISABLE_VELLO=1".to_string(),
            ));
        }

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            dx12_shader_compiler: wgpu::Dx12Compiler::Fxc,
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::None,
                compatible_surface: None,
                force_fallback_adapter: true,
            })
            .await
            .ok_or_else(|| RendererError::Failed("No compatible GPU adapter".to_string()))?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_defaults(),
                },
                None,
            )
            .await
            .map_err(|e| RendererError::Failed(format!("Device creation failed: {e}")))?;

        let renderer = vello::Renderer::new(
            &device,
            RendererOptions {
                surface_format: None,
                use_cpu: true,
                antialiasing_support: AaSupport::area_only(),
                num_init_threads: NonZeroUsize::new(1),
            },
        )
        .map_err(|e| RendererError::Failed(format!("Vello init failed: {e}")))?;

        Ok(Self {
            device,
            queue,
            renderer,
            text_engine: TextEngine::new(),
            images: HashMap::new(),
            svg_cache: RefCell::new(HashMap::new()),
            current_time: 0.0,
        })
    }

    /// Select the `peniko::Image` to draw for an asset id, advancing animated
    /// GIFs to the frame whose delay window contains the current time. SVGs are
    /// rasterized (and cached) at the requested size.
    fn select_image(
        &self,
        asset_id: &str,
        want_w: Option<f32>,
        want_h: Option<f32>,
    ) -> Option<Image> {
        match self.images.get(asset_id)? {
            VelloAsset::Static(img) => Some(img.clone()),
            VelloAsset::Animated {
                frames,
                delays_ms,
                total_ms,
            } => {
                if *total_ms == 0 {
                    return frames.first().cloned();
                }
                let t_ms = (self.current_time.max(0.0) * 1000.0) as u32 % *total_ms;
                let mut acc = 0u32;
                for (i, d) in delays_ms.iter().enumerate() {
                    acc += (*d).max(1);
                    if t_ms < acc {
                        return frames.get(i).cloned();
                    }
                }
                frames.last().cloned()
            }
            VelloAsset::Svg(tree) => self.rasterize_svg_image(asset_id, tree, want_w, want_h),
        }
    }

    /// Rasterize an SVG tree to a straight-alpha `peniko::Image` at the requested
    /// size, caching by (id, w, h) since rasterization is expensive.
    fn rasterize_svg_image(
        &self,
        asset_id: &str,
        tree: &resvg::usvg::Tree,
        want_w: Option<f32>,
        want_h: Option<f32>,
    ) -> Option<Image> {
        let size = tree.size();
        let tw = want_w.unwrap_or_else(|| size.width()).round().max(1.0) as u32;
        let th = want_h.unwrap_or_else(|| size.height()).round().max(1.0) as u32;
        let key = (asset_id.to_string(), tw, th);
        if let Some(img) = self.svg_cache.borrow().get(&key) {
            return Some(img.clone());
        }
        let mut pm = tiny_skia::Pixmap::new(tw, th)?;
        let sx = tw as f32 / size.width();
        let sy = th as f32 / size.height();
        resvg::render(
            tree,
            tiny_skia::Transform::from_scale(sx, sy),
            &mut pm.as_mut(),
        );
        let img = rgba_to_image(raster::pixmap_to_straight_rgba(&pm), tw, th);
        self.svg_cache.borrow_mut().insert(key, img.clone());
        Some(img)
    }

    fn build_scene(
        &self,
        objects: &HashMap<String, Object>,
        width: u32,
        height: u32,
        background: &str,
        camera: Option<&CameraState>,
    ) -> Result<Scene, RendererError> {
        let mut scene = Scene::new();

        // Fill background
        let bg = parse_vello_color(background, 1.0);
        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            bg,
            None,
            &Rect::new(0.0, 0.0, width as f64, height as f64),
        );

        // Camera root transform (shared with the CPU backend so the
        // matrices are bit-identical).
        let root = crate::common::scene::camera_transform(camera, width, height);

        let ctx = WalkCtx {
            objects,
            canvas: (width, height),
        };
        for id in crate::common::scene::sorted_root_ids(objects) {
            self.draw_node(&mut scene, id, &ctx, root, 0)?;
        }

        Ok(scene)
    }

    /// Walk one node of the scene graph, recursing through groups.
    ///
    /// `depth` bounds the recursion for the same reason the CPU backend does:
    /// a deep group chain would overflow the stack, which aborts the process.
    fn draw_node(
        &self,
        scene: &mut Scene,
        id: &str,
        ctx: &WalkCtx<'_>,
        parent: crate::common::scene::Mat2x3,
        depth: usize,
    ) -> Result<(), RendererError> {
        if depth > luminafx_core::validation::MAX_GROUP_DEPTH {
            return Err(RendererError::Failed(format!(
                "group nesting deeper than {} at '{id}'",
                luminafx_core::validation::MAX_GROUP_DEPTH
            )));
        }
        // An error, as on the CPU backend. Skipping the missing object drew the
        // rest of the frame without it, so the same scene failed on one backend
        // and rendered on the other.
        let obj = ctx.objects.get(id).ok_or_else(|| {
            RendererError::Failed(format!("Object '{id}' not found in scene graph"))
        })?;
        match obj {
            Object::Group(props) => {
                let transform = crate::common::scene::group_transform(parent, props);

                for child_id in crate::common::scene::sorted_children(&props.children, ctx.objects)
                {
                    self.draw_node(scene, child_id, ctx, transform, depth + 1)?;
                }
                Ok(())
            }
            _ => self.draw_leaf(scene, obj, parent, ctx.canvas, ctx.objects),
        }
    }

    /// Emit one leaf object.
    ///
    /// Returns `Err` on the same malformed input the CPU backend rejects.
    /// The two backends must agree on failure as well as on pixels: when one
    /// aborted the export and the other silently skipped the object, the same
    /// scene produced different output depending on `--backend`, and the
    /// pixel-diff suite could not see it — it only compares frames that both
    /// backends produced.
    fn draw_leaf(
        &self,
        scene: &mut Scene,
        obj: &Object,
        mat: crate::common::scene::Mat2x3,
        canvas: (u32, u32),
        objects: &HashMap<String, Object>,
    ) -> Result<(), RendererError> {
        let affine = mat.to_kurbo();
        match obj {
            Object::Circle(props) => {
                let (cx, cy) = (f64::from(props.cx), f64::from(props.cy));
                let radius = f64::from(props.radius);
                if radius <= 0.0 {
                    return Ok(());
                }
                let opacity = props.opacity;

                let circle = Circle::new((cx, cy), radius);
                let bbox = (
                    (cx - radius) as f32,
                    (cy - radius) as f32,
                    (radius * 2.0) as f32,
                    (radius * 2.0) as f32,
                );
                if let Some(spec) = props.shadow.as_ref().map(shadow_spec) {
                    let mut pb = tiny_skia::PathBuilder::new();
                    pb.push_circle(cx as f32, cy as f32, radius as f32);
                    if let Some(path) = pb.finish() {
                        draw_shadow_image(scene, canvas, &path, mat, &spec);
                    }
                }
                let fill = props.fill.as_ref().map(|paint| {
                    fill_spec(paint, opacity).unwrap_or_else(|| FillSpec::solid("#FFFFFF", opacity))
                });
                if let Some(fill) = &fill {
                    scene.fill(
                        Fill::NonZero,
                        affine,
                        &brush_from_fill(fill, bbox),
                        None,
                        &circle,
                    );
                }

                if let Some(stroke) = props
                    .stroke
                    .as_ref()
                    .and_then(|paint| fill_spec(paint, opacity))
                {
                    scene.stroke(
                        &flat_stroke(f64::from(props.stroke_width)),
                        affine,
                        &brush_from_fill(&stroke, bbox),
                        None,
                        &circle,
                    );
                }
            }
            Object::Rectangle(props) => {
                let (x, y) = (f64::from(props.x), f64::from(props.y));
                let (w, h) = (f64::from(props.width), f64::from(props.height));
                if w <= 0.0 || h <= 0.0 {
                    return Ok(());
                }
                let opacity = props.opacity;

                let rx = props.rx;
                let ry = if props.ry > 0.0 { props.ry } else { rx };
                let bbox = (x as f32, y as f32, w as f32, h as f32);
                let fill = props.fill.as_ref().map(|paint| {
                    fill_spec(paint, opacity).unwrap_or_else(|| FillSpec::solid("#FFFFFF", opacity))
                });
                let stroke = props
                    .stroke
                    .as_ref()
                    .and_then(|paint| fill_spec(paint, opacity));
                let sw = f64::from(props.stroke_width);

                if let Some(spec) = props.shadow.as_ref().map(shadow_spec) {
                    let tiny_path = if rx > 0.0 {
                        crate::common::path::to_tiny_path(&crate::common::path::rounded_rect(
                            x as f32, y as f32, w as f32, h as f32, rx, ry,
                        ))
                    } else {
                        tiny_skia::Rect::from_xywh(x as f32, y as f32, w as f32, h as f32).and_then(
                            |r| {
                                let mut pb = tiny_skia::PathBuilder::new();
                                pb.push_rect(r);
                                pb.finish()
                            },
                        )
                    };
                    if let Some(path) = tiny_path {
                        draw_shadow_image(scene, canvas, &path, mat, &spec);
                    }
                }
                if rx > 0.0 {
                    // Rounded corners: same quadratic-arc geometry as the CPU backend.
                    let path =
                        crate::common::path::to_kurbo_path(&crate::common::path::rounded_rect(
                            x as f32, y as f32, w as f32, h as f32, rx, ry,
                        ));
                    if let Some(fill) = &fill {
                        scene.fill(
                            Fill::NonZero,
                            affine,
                            &brush_from_fill(fill, bbox),
                            None,
                            &path,
                        );
                    }
                    if let Some(s) = &stroke {
                        scene.stroke(
                            &flat_stroke(sw),
                            affine,
                            &brush_from_fill(s, bbox),
                            None,
                            &path,
                        );
                    }
                } else {
                    let rect = Rect::new(x, y, x + w, y + h);
                    if let Some(fill) = &fill {
                        scene.fill(
                            Fill::NonZero,
                            affine,
                            &brush_from_fill(fill, bbox),
                            None,
                            &rect,
                        );
                    }
                    if let Some(s) = &stroke {
                        scene.stroke(
                            &flat_stroke(sw),
                            affine,
                            &brush_from_fill(s, bbox),
                            None,
                            &rect,
                        );
                    }
                }
            }
            Object::Line(props) => {
                let (x1, y1) = (f64::from(props.x1), f64::from(props.y1));
                let (x2, y2) = (f64::from(props.x2), f64::from(props.y2));
                let stroke_color = parse_vello_color(&props.stroke, props.opacity);

                let line = Line::new((x1, y1), (x2, y2));
                let mut stroke = flat_stroke(f64::from(props.stroke_width));
                if let Some(frac) = props.draw_fraction {
                    // Same partial-reveal dash the CPU backend uses.
                    let dx = (x2 - x1) as f32;
                    let dy = (y2 - y1) as f32;
                    let length = (dx * dx + dy * dy).sqrt().max(0.001);
                    let dashes = crate::common::stroke::draw_fraction_dash(frac, length);
                    stroke = stroke.with_dashes(0.0, dashes.iter().map(|d| f64::from(*d)));
                } else if let Some(pattern) =
                    crate::common::stroke::dash_pattern(props.dash.as_deref())
                {
                    // Shared with the CPU backend so both read and normalise
                    // the pattern identically (TD-19).
                    stroke = stroke.with_dashes(0.0, pattern.iter().map(|d| f64::from(*d)));
                }
                scene.stroke(&stroke, affine, stroke_color, None, &line);
            }
            Object::Arrow(props) => {
                // `[f32; 2]` endpoints: the malformed-array errors this branch
                // raised (#53) cannot be expressed any more.
                let (fx, fy) = (f64::from(props.from[0]), f64::from(props.from[1]));
                let (tx, ty) = (f64::from(props.to[0]), f64::from(props.to[1]));
                let sw = f64::from(props.stroke_width);
                let color = parse_vello_color(&props.color, props.opacity);

                let line = Line::new((fx, fy), (tx, ty));
                scene.stroke(&flat_stroke(sw), affine, color, None, &line);

                // Arrowhead
                let dx = tx - fx;
                let dy = ty - fy;
                let angle = dy.atan2(dx);
                let head_len = (sw * 5.0).max(10.0);
                let head_angle = std::f64::consts::PI / 6.0;

                let mut head = BezPath::new();
                head.move_to((tx, ty));
                head.line_to((
                    tx - head_len * (angle - head_angle).cos(),
                    ty - head_len * (angle - head_angle).sin(),
                ));
                head.move_to((tx, ty));
                head.line_to((
                    tx - head_len * (angle + head_angle).cos(),
                    ty - head_len * (angle + head_angle).sin(),
                ));
                scene.stroke(&flat_stroke(sw), affine, color, None, &head);
            }
            Object::BezierCurve(props) => {
                let stroke_color = parse_vello_color(&props.stroke, props.opacity);

                // Trimmed by arc length through the shared helper, so both
                // backends reveal the same portion of the same curve. Cutting
                // at parameter `t` was exact but measured the wrong quantity:
                // a cubic traversed at uniform `t` does not move at uniform
                // speed.
                let [p0, p1, p2, p3] =
                    [props.p0, props.p1, props.p2, props.p3].map(|[x, y]| (x, y));
                let curve = crate::common::path::PathData::cubic(p0, p1, p2, p3);
                let curve = crate::common::path::trim(&curve, props.draw_fraction.unwrap_or(1.0));
                let path = crate::common::path::to_kurbo_path(&curve);
                scene.stroke(
                    &flat_stroke(f64::from(props.stroke_width)),
                    affine,
                    stroke_color,
                    None,
                    &path,
                );
            }
            Object::Polygon(props) => {
                let opacity = props.opacity;

                let mut path = BezPath::new();
                let (mut min_x, mut min_y) = (f32::INFINITY, f32::INFINITY);
                let (mut max_x, mut max_y) = (f32::NEG_INFINITY, f32::NEG_INFINITY);
                for (i, &[x, y]) in props.points.iter().enumerate() {
                    min_x = min_x.min(x);
                    min_y = min_y.min(y);
                    max_x = max_x.max(x);
                    max_y = max_y.max(y);
                    let point = (f64::from(x), f64::from(y));
                    if i == 0 {
                        path.move_to(point);
                    } else {
                        path.line_to(point);
                    }
                }
                path.close_path();
                if !min_x.is_finite() {
                    return Ok(());
                }
                let bbox = (min_x, min_y, max_x - min_x, max_y - min_y);

                if let Some(spec) = props.shadow.as_ref().map(shadow_spec) {
                    let mut pb = tiny_skia::PathBuilder::new();
                    for (i, &[px, py]) in props.points.iter().enumerate() {
                        if i == 0 {
                            pb.move_to(px, py);
                        } else {
                            pb.line_to(px, py);
                        }
                    }
                    pb.close();
                    if let Some(tiny_path) = pb.finish() {
                        draw_shadow_image(scene, canvas, &tiny_path, mat, &spec);
                    }
                }
                let fill = props.fill.as_ref().map(|paint| {
                    fill_spec(paint, opacity).unwrap_or_else(|| FillSpec::solid("#FFFFFF", opacity))
                });
                if let Some(fill) = &fill {
                    scene.fill(
                        Fill::NonZero,
                        affine,
                        &brush_from_fill(fill, bbox),
                        None,
                        &path,
                    );
                }

                if let Some(stroke) = props
                    .stroke
                    .as_ref()
                    .and_then(|paint| fill_spec(paint, opacity))
                {
                    scene.stroke(
                        &flat_stroke(f64::from(props.stroke_width)),
                        affine,
                        &brush_from_fill(&stroke, bbox),
                        None,
                        &path,
                    );
                }
            }
            Object::Path(props) => {
                let opacity = props.opacity;

                if let Some(data) = crate::common::path::parse_svg_path(&props.d) {
                    // Shared arc-length trim, matching the CPU backend.
                    let data = match props.draw_fraction {
                        Some(frac) => crate::common::path::trim(&data, frac),
                        None => data,
                    };
                    let path = crate::common::path::to_kurbo_path(&data);
                    let bbox = crate::common::path::bbox(&data).unwrap_or((0.0, 0.0, 0.0, 0.0));
                    if let Some(spec) = props.shadow.as_ref().map(shadow_spec) {
                        if let Some(tiny_path) = crate::common::path::to_tiny_path(&data) {
                            draw_shadow_image(scene, canvas, &tiny_path, mat, &spec);
                        }
                    }
                    if let Some(fill) = props
                        .fill
                        .as_ref()
                        .and_then(|paint| fill_spec(paint, opacity))
                    {
                        scene.fill(
                            Fill::NonZero,
                            affine,
                            &brush_from_fill(&fill, bbox),
                            None,
                            &path,
                        );
                    }
                    if let Some(stroke) = props
                        .stroke
                        .as_ref()
                        .and_then(|paint| fill_spec(paint, opacity))
                    {
                        scene.stroke(
                            &flat_stroke(f64::from(props.stroke_width)),
                            affine,
                            &brush_from_fill(&stroke, bbox),
                            None,
                            &path,
                        );
                    }
                }
            }
            Object::NumberLine(props) => {
                let (start, end) = (f64::from(props.start), f64::from(props.end));
                let step = f64::from(props.step);
                let (x, y) = (f64::from(props.x), f64::from(props.y));
                let length = f64::from(props.length.unwrap_or(400.0));
                let color = parse_vello_color(&props.color, props.opacity);
                let range = end - start;
                if range <= 0.0 || step <= 0.0 {
                    return Ok(());
                }

                let stroke = flat_stroke(2.0);
                let main = Line::new((x, y), (x + length, y));
                scene.stroke(&stroke, affine, color, None, &main);

                let tick_h = 8.0;
                for i in 0..crate::common::ticks::count(start as f32, end as f32, step as f32) {
                    let t = f64::from(crate::common::ticks::at(start as f32, step as f32, i));
                    let px = x + (t - start) / range * length;
                    let tick = Line::new((px, y - tick_h), (px, y + tick_h));
                    scene.stroke(&stroke, affine, color, None, &tick);
                }
            }
            Object::Axes(props) => {
                let (x, y) = (f64::from(props.x), f64::from(props.y));
                let color = parse_vello_color(&props.color, props.opacity);
                let scale = f64::from(props.scale);
                let (x_step, y_step) = (f64::from(props.x_step), f64::from(props.y_step));
                let draw_grid = props.grid;
                let [x_min, x_max] = props.x_range.map(f64::from);
                let [y_min, y_max] = props.y_range.map(f64::from);

                let ox = x + (0.0 - x_min) * scale;
                let oy = y - (0.0 - y_min) * scale;

                // Ticks use the axis stroke width (2.0) exactly like the CPU
                // backend; only grid lines are thin.
                let axis_stroke = flat_stroke(2.0);
                let grid_stroke = flat_stroke(1.0);
                let grid_color =
                    Color::rgba8(color.r, color.g, color.b, ((color.a as f32) * 0.2) as u8);

                // X axis
                scene.stroke(
                    &axis_stroke,
                    affine,
                    color,
                    None,
                    &Line::new((ox + x_min * scale, oy), (ox + x_max * scale, oy)),
                );
                // Y axis
                scene.stroke(
                    &axis_stroke,
                    affine,
                    color,
                    None,
                    &Line::new((ox, oy - y_min * scale), (ox, oy - y_max * scale)),
                );

                // X ticks and optional vertical grid lines
                let x_count =
                    crate::common::ticks::count(x_min as f32, x_max as f32, x_step as f32);
                for i in 0..x_count {
                    let tx = f64::from(crate::common::ticks::at(x_min as f32, x_step as f32, i));
                    if tx > x_max + 1e-4 {
                        break;
                    }
                    let px = ox + tx * scale;
                    scene.stroke(
                        &axis_stroke,
                        affine,
                        color,
                        None,
                        &Line::new((px, oy - 5.0), (px, oy + 5.0)),
                    );
                    if draw_grid && (tx - 0.0).abs() > 1e-4 {
                        scene.stroke(
                            &grid_stroke,
                            affine,
                            grid_color,
                            None,
                            &Line::new((px, oy - y_min * scale), (px, oy - y_max * scale)),
                        );
                    }
                }

                // Y ticks and optional horizontal grid lines
                let y_count =
                    crate::common::ticks::count(y_min as f32, y_max as f32, y_step as f32);
                for i in 0..y_count {
                    let ty = f64::from(crate::common::ticks::at(y_min as f32, y_step as f32, i));
                    if ty > y_max + 1e-4 {
                        break;
                    }
                    let py = oy - ty * scale;
                    scene.stroke(
                        &axis_stroke,
                        affine,
                        color,
                        None,
                        &Line::new((ox - 5.0, py), (ox + 5.0, py)),
                    );
                    if draw_grid && (ty - 0.0).abs() > 1e-4 {
                        scene.stroke(
                            &grid_stroke,
                            affine,
                            grid_color,
                            None,
                            &Line::new((ox + x_min * scale, py), (ox + x_max * scale, py)),
                        );
                    }
                }
            }
            Object::Plot(props) => {
                let function_str = props.function_str.as_str();
                let sw = f64::from(props.stroke_width);
                let color = parse_vello_color(&props.color, props.opacity);
                let samples = props.sample_count as usize;
                let draw_fraction = props.draw_fraction.unwrap_or(1.0);

                // A plot drawn against anything but an Axes draws nothing, as
                // on the CPU backend; validation reports AXES_ID_IS_NOT_AXES.
                // This backend used to read such an object's missing ranges as
                // -10..10 and draw the curve anyway.
                let Some(Object::Axes(axes)) = objects.get(&props.axes_id) else {
                    return Ok(());
                };
                let (x, y) = (f64::from(axes.x), f64::from(axes.y));
                let [x_min, x_max] = axes.x_range.map(f64::from);
                let [y_min, y_max] = axes.y_range.map(f64::from);
                let scale = f64::from(axes.scale);
                let ox = x + (0.0 - x_min) * scale;
                let oy = y - (0.0 - y_min) * scale;
                let x_end = x_min + (x_max - x_min) * draw_fraction as f64;

                // Sampling is shared with the CPU backend so both draw the
                // same curve from the same points (TD-02): where to sample is a
                // rendering decision, and only the emitting differs.
                let segments =
                    crate::common::plot::sample(function_str, x_min, x_end, y_min, y_max, samples);

                let mut path = BezPath::new();
                for segment in &segments {
                    for (i, (mx, my)) in segment.iter().enumerate() {
                        let sx = ox + mx * scale;
                        let sy = oy - my * scale;
                        if i == 0 {
                            path.move_to((sx, sy));
                        } else {
                            path.line_to((sx, sy));
                        }
                    }
                }
                if !path.is_empty() {
                    scene.stroke(&KurboStroke::new(sw), affine, color, None, &path);
                }
            }
            Object::Text(_) | Object::LaTeX(_) | Object::MathML(_) => {
                let Some((text, style)) = crate::common::text::text_of(obj) else {
                    return Ok(());
                };

                // One image per glyph, at the position the shared layout
                // gives — the same bitmap the CPU backend composites, in the
                // same place, resampled once rather than twice (TD-18).
                for g in raster::rasterize_glyphs(
                    &self.text_engine,
                    &text,
                    style.font_size,
                    style.color,
                    style.font_id,
                    style.align,
                    style.letter_spacing,
                    style.opacity,
                    style.x,
                    style.y,
                ) {
                    let img = rgba_to_image(g.rgba, g.width, g.height);
                    let t = affine * Affine::translate(Vec2::new(f64::from(g.ix), f64::from(g.iy)));
                    scene.draw_image(&img, t);
                }
            }
            Object::Image(_) | Object::SVG(_) => {
                let Some(AssetPlacement {
                    asset_id,
                    x,
                    y,
                    width: want_w,
                    height: want_h,
                    rotation,
                    opacity,
                }) = asset_placement(obj)
                else {
                    return Ok(());
                };
                if asset_id.is_empty() {
                    return Ok(());
                }
                let (x, y) = (f64::from(x), f64::from(y));
                // SVGs rasterize directly to the requested size (1:1 composite);
                // raster images scale from their natural size.
                let is_svg = matches!(self.images.get(asset_id), Some(VelloAsset::Svg(_)));
                if let Some(img) = self.select_image(asset_id, want_w, want_h) {
                    let sw = img.width as f64;
                    let sh = img.height as f64;
                    let (scale_x, scale_y) = if is_svg {
                        (1.0, 1.0)
                    } else {
                        (
                            want_w.map(|w| w as f64 / sw).unwrap_or(1.0),
                            want_h.map(|h| h as f64 / sh).unwrap_or(1.0),
                        )
                    };
                    let dw = sw * scale_x;
                    let dh = sh * scale_y;
                    let mut local = Affine::translate(Vec2::new(x, y));
                    if rotation != 0.0 {
                        local *= Affine::translate(Vec2::new(dw / 2.0, dh / 2.0))
                            * Affine::rotate((rotation as f64).to_radians())
                            * Affine::translate(Vec2::new(-dw / 2.0, -dh / 2.0));
                    }
                    local *= Affine::scale_non_uniform(scale_x, scale_y);
                    let placement = affine * local;
                    // draw_image has no alpha channel, so wrap in an alpha layer
                    // (clipped to the image rect) when the object is translucent.
                    let translucent = opacity < 0.999;
                    if translucent {
                        scene.push_layer(
                            BlendMode::new(Mix::Normal, Compose::SrcOver),
                            opacity,
                            placement,
                            &Rect::new(0.0, 0.0, sw, sh),
                        );
                    }
                    scene.draw_image(&img, placement);
                    if translucent {
                        scene.pop_layer();
                    }
                }
            }
            Object::Particles(props) => {
                if props.count == 0 {
                    return Ok(());
                }
                let base = parse_vello_color(&props.color, 1.0);
                for dot in raster::simulate_particles(
                    props.count,
                    props.emitter_x,
                    props.emitter_y,
                    props.lifetime,
                    props.speed,
                    props.spread,
                    props.size,
                    props.opacity,
                    self.current_time,
                ) {
                    let c = Color::rgba8(base.r, base.g, base.b, (dot.alpha * 255.0) as u8);
                    scene.fill(
                        Fill::NonZero,
                        affine,
                        c,
                        None,
                        &Circle::new((dot.x as f64, dot.y as f64), dot.r as f64),
                    );
                }
            }
            Object::Group(_) => {} // handled in draw_node
        }
        Ok(())
    }
}

impl Renderer for VelloRenderer {
    fn render_frame(
        &mut self,
        objects: &HashMap<String, Object>,
        width: u32,
        height: u32,
        background: &str,
        camera: Option<&CameraState>,
    ) -> Result<Vec<u8>, RendererError> {
        let scene = self.build_scene(objects, width, height, background, camera)?;

        // Create render target texture (Rgba8Unorm with STORAGE_BINDING + COPY_SRC)
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("vello_render_target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Parse background color for vello RenderParams
        let bg = parse_vello_color(background, 1.0);

        self.renderer
            .render_to_texture(
                &self.device,
                &self.queue,
                &scene,
                &texture_view,
                &RenderParams {
                    base_color: bg,
                    width,
                    height,
                    antialiasing_method: AaConfig::Area,
                },
            )
            .map_err(|e| RendererError::Failed(format!("Vello render failed: {e}")))?;

        // Copy rendered texture to a CPU-readable staging buffer
        let bytes_per_pixel = 4u32;
        let unpadded_bytes_per_row = width * bytes_per_pixel;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;
        let buffer_size = (padded_bytes_per_row * height) as u64;

        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("vello_staging"),
            size: buffer_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("vello_readback"),
            });
        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &staging_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(Some(encoder.finish()));

        // Map the staging buffer synchronously
        let buffer_slice = staging_buffer.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            sender.send(result).ok();
        });
        self.device.poll(wgpu::Maintain::Wait);
        receiver
            .recv()
            .map_err(|_| RendererError::Failed("Buffer mapping channel closed".to_string()))?
            .map_err(|e| RendererError::Failed(format!("Buffer mapping failed: {e}")))?;

        // Unpad rows: extract only the active bytes (skip padding at end of each row)
        let mapped = buffer_slice.get_mapped_range();
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);
        for row in 0..height {
            let start = (row * padded_bytes_per_row) as usize;
            let end = start + unpadded_bytes_per_row as usize;
            pixels.extend_from_slice(&mapped[start..end]);
        }
        drop(mapped);
        staging_buffer.unmap();

        Ok(pixels)
    }

    fn load_font(&mut self, id: &str, data: &[u8]) -> Result<(), RendererError> {
        // GPU text reuses the fontdue raster path (rasterized glyphs composited
        // via draw_image), so the same TextEngine as the CPU backend is loaded.
        self.text_engine
            .load_font(id.to_string(), data)
            .map_err(RendererError::Failed)
    }

    fn load_image(&mut self, id: &str, data: &[u8]) -> Result<(), RendererError> {
        if looks_like_svg(data) {
            let opt = resvg::usvg::Options::default();
            let tree = resvg::usvg::Tree::from_data(data, &opt)
                .map_err(|e| RendererError::Failed(format!("SVG parse failed for '{id}': {e}")))?;
            self.images
                .insert(id.to_string(), VelloAsset::Svg(Box::new(tree)));
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
                let mut imgs = Vec::with_capacity(frames.len());
                let mut delays = Vec::with_capacity(frames.len());
                let mut total = 0u32;
                for f in frames {
                    let (num, den) = f.delay().numer_denom_ms();
                    let ms = if den == 0 { num } else { num / den.max(1) }.max(1);
                    let buf = f.into_buffer();
                    let (w, h) = buf.dimensions();
                    imgs.push(rgba_to_image(buf.into_raw(), w, h));
                    delays.push(ms);
                    total += ms;
                }
                self.images.insert(
                    id.to_string(),
                    VelloAsset::Animated {
                        frames: imgs,
                        delays_ms: delays,
                        total_ms: total,
                    },
                );
                return Ok(());
            }
            if let Some(f) = frames.into_iter().next() {
                let buf = f.into_buffer();
                let (w, h) = buf.dimensions();
                self.images.insert(
                    id.to_string(),
                    VelloAsset::Static(rgba_to_image(buf.into_raw(), w, h)),
                );
                return Ok(());
            }
        }

        // Static raster (PNG/JPEG/WebP/...).
        let img = image::load_from_memory(data)
            .map_err(|e| RendererError::Failed(format!("Image decode failed for '{id}': {e}")))?
            .to_rgba8();
        let (w, h) = img.dimensions();
        self.images.insert(
            id.to_string(),
            VelloAsset::Static(rgba_to_image(img.into_raw(), w, h)),
        );
        Ok(())
    }

    fn set_time(&mut self, time: f32) {
        self.current_time = time;
    }
}

/// Wrap straight-alpha RGBA8 bytes in a `peniko::Image` for `draw_image`.
fn rgba_to_image(rgba: Vec<u8>, width: u32, height: u32) -> Image {
    Image::new(Blob::from(rgba), Format::Rgba8, width, height)
}

/// Stroke matching tiny-skia's `Stroke::default()`: butt caps, miter joins
/// (limit 4). kurbo defaults to round caps/joins, which visibly diverges
/// from the CPU backend at line ends and sharp corners.
fn flat_stroke(width: f64) -> KurboStroke {
    KurboStroke::new(width)
        .with_caps(Cap::Butt)
        .with_join(Join::Miter)
        .with_miter_limit(4.0)
}

/// Adapt a shared `FillSpec` to a peniko brush, deriving gradient geometry
/// from the same bbox numbers as the CPU backend (`common::fill`).
fn brush_from_fill(fill: &crate::common::fill::FillSpec, bbox: (f32, f32, f32, f32)) -> Brush {
    use crate::common::fill::FillSpec;
    match fill {
        FillSpec::Solid(c) => Brush::Solid(crate::common::color::to_peniko(*c)),
        FillSpec::Linear { stops, angle_deg } => {
            let (start, end) = crate::common::fill::linear_geometry(bbox, *angle_deg);
            Brush::Gradient(
                Gradient::new_linear(
                    (start.0 as f64, start.1 as f64),
                    (end.0 as f64, end.1 as f64),
                )
                .with_stops(peniko_stops(stops).as_slice()),
            )
        }
        FillSpec::Radial { stops, radius_frac } => {
            let (center, r) = crate::common::fill::radial_geometry(bbox, *radius_frac);
            Brush::Gradient(
                Gradient::new_radial((center.0 as f64, center.1 as f64), r)
                    .with_stops(peniko_stops(stops).as_slice()),
            )
        }
    }
}

fn peniko_stops(stops: &[(f32, [u8; 4])]) -> Vec<ColorStop> {
    stops
        .iter()
        .map(|(p, c)| ColorStop::from((*p, crate::common::color::to_peniko(*c))))
        .collect()
}

/// Composite a drop shadow into the scene: rasterize the shared blurred
/// silhouette (identical bytes to the CPU backend) and draw it as a
/// full-canvas image under an opacity layer.
fn draw_shadow_image(
    scene: &mut Scene,
    canvas: (u32, u32),
    path: &tiny_skia::Path,
    mat: crate::common::scene::Mat2x3,
    spec: &crate::common::shadow::ShadowSpec,
) {
    let pm =
        match crate::common::shadow::shadow_pixmap(canvas.0, canvas.1, path, mat.to_tiny(), spec) {
            Some(p) => p,
            None => return,
        };
    let img = rgba_to_image(raster::pixmap_to_straight_rgba(&pm), canvas.0, canvas.1);
    let full = Rect::new(0.0, 0.0, canvas.0 as f64, canvas.1 as f64);
    scene.push_layer(
        BlendMode::new(Mix::Normal, Compose::SrcOver),
        spec.opacity.clamp(0.0, 1.0),
        Affine::IDENTITY,
        &full,
    );
    scene.draw_image(&img, Affine::IDENTITY);
    scene.pop_layer();
}

/// Heuristic: does this byte slice look like an SVG document?
fn looks_like_svg(data: &[u8]) -> bool {
    let head = &data[..data.len().min(512)];
    let s = String::from_utf8_lossy(head);
    let trimmed = s.trim_start();
    trimmed.starts_with("<?xml") || trimmed.starts_with("<svg") || s.contains("<svg")
}

fn parse_vello_color(hex: &str, opacity: f32) -> Color {
    crate::common::color::to_peniko(crate::common::color::parse_rgba8(hex, opacity))
}

/// Where an `Image` or `SVG` object places its asset. The two props types
/// have the same fields, and this backend draws both through one path.
struct AssetPlacement<'a> {
    asset_id: &'a str,
    x: f32,
    y: f32,
    width: Option<f32>,
    height: Option<f32>,
    rotation: f32,
    opacity: f32,
}

fn asset_placement(object: &Object) -> Option<AssetPlacement<'_>> {
    macro_rules! placement {
        ($props:expr) => {
            AssetPlacement {
                asset_id: &$props.asset_id,
                x: $props.x,
                y: $props.y,
                width: $props.width,
                height: $props.height,
                rotation: $props.rotation,
                opacity: $props.opacity,
            }
        };
    }
    match object {
        Object::Image(props) => Some(placement!(props)),
        Object::SVG(props) => Some(placement!(props)),
        _ => None,
    }
}
