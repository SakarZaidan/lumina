use crate::raster;
use crate::{Renderer, RendererError};
use lumina_schema::{CameraState, Object};
use lumina_text::TextEngine;
use serde_json::Value;
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

pub struct VelloRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: vello::Renderer,
    text_engine: TextEngine,
    images: HashMap<String, VelloAsset>,
    svg_cache: RefCell<HashMap<(String, u32, u32), Image>>,
    current_time: f32,
}

impl VelloRenderer {
    /// Create a new VelloRenderer, blocking the current thread during GPU init.
    pub fn new() -> Result<Self, RendererError> {
        pollster::block_on(Self::new_async())
    }

    async fn new_async() -> Result<Self, RendererError> {
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
        states: &HashMap<String, Value>,
        width: u32,
        height: u32,
        background: &str,
        camera: Option<&CameraState>,
    ) -> Scene {
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

        for id in crate::common::scene::sorted_root_ids(objects) {
            self.draw_node(&mut scene, &id, objects, states, root);
        }

        scene
    }

    fn draw_node(
        &self,
        scene: &mut Scene,
        id: &str,
        objects: &HashMap<String, Object>,
        states: &HashMap<String, Value>,
        parent: crate::common::scene::Mat2x3,
    ) {
        let obj = match objects.get(id) {
            Some(o) => o,
            None => return,
        };
        let state = match states.get(id) {
            Some(s) => s,
            None => return,
        };

        match obj {
            Object::Group(props) => {
                let transform = crate::common::scene::group_transform(parent, state);

                for child_id in crate::common::scene::sorted_children(&props.children, objects) {
                    self.draw_node(scene, child_id, objects, states, transform);
                }
            }
            _ => self.draw_leaf(scene, obj, state, parent.to_kurbo(), objects, states),
        }
    }

    fn draw_leaf(
        &self,
        scene: &mut Scene,
        obj: &Object,
        state: &Value,
        affine: Affine,
        _objects: &HashMap<String, Object>,
        states: &HashMap<String, Value>,
    ) {
        match obj {
            Object::Circle(_) => {
                let cx = state["cx"].as_f64().unwrap_or(0.0);
                let cy = state["cy"].as_f64().unwrap_or(0.0);
                let radius = state["radius"].as_f64().unwrap_or(0.0);
                if radius <= 0.0 {
                    return;
                }
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;

                let circle = Circle::new((cx, cy), radius);
                let bbox = (
                    (cx - radius) as f32,
                    (cy - radius) as f32,
                    (radius * 2.0) as f32,
                    (radius * 2.0) as f32,
                );
                let fill = crate::common::fill::parse_fill(&state["fill"], opacity)
                    .unwrap_or_else(|| crate::common::fill::FillSpec::solid("#FFFFFF", opacity));
                scene.fill(
                    Fill::NonZero,
                    affine,
                    &brush_from_fill(&fill, bbox),
                    None,
                    &circle,
                );

                if let Some(stroke) = crate::common::fill::parse_stroke(state, opacity) {
                    let sw = state["stroke_width"].as_f64().unwrap_or(1.0);
                    scene.stroke(
                        &flat_stroke(sw),
                        affine,
                        &brush_from_fill(&stroke, bbox),
                        None,
                        &circle,
                    );
                }
            }
            Object::Rectangle(_) => {
                let x = state["x"].as_f64().unwrap_or(0.0);
                let y = state["y"].as_f64().unwrap_or(0.0);
                let w = state["width"].as_f64().unwrap_or(0.0);
                let h = state["height"].as_f64().unwrap_or(0.0);
                if w <= 0.0 || h <= 0.0 {
                    return;
                }
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;

                let rx = state["rx"].as_f64().unwrap_or(0.0) as f32;
                let ry_raw = state["ry"].as_f64().unwrap_or(0.0) as f32;
                let ry = if ry_raw > 0.0 { ry_raw } else { rx };
                let bbox = (x as f32, y as f32, w as f32, h as f32);
                let fill = crate::common::fill::parse_fill(&state["fill"], opacity)
                    .unwrap_or_else(|| crate::common::fill::FillSpec::solid("#FFFFFF", opacity));
                let stroke = crate::common::fill::parse_stroke(state, opacity);
                let sw = state["stroke_width"].as_f64().unwrap_or(1.0);

                if rx > 0.0 {
                    // Rounded corners: same quadratic-arc geometry as the CPU backend.
                    let path =
                        crate::common::path::to_kurbo_path(&crate::common::path::rounded_rect(
                            x as f32, y as f32, w as f32, h as f32, rx, ry,
                        ));
                    scene.fill(
                        Fill::NonZero,
                        affine,
                        &brush_from_fill(&fill, bbox),
                        None,
                        &path,
                    );
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
                    scene.fill(
                        Fill::NonZero,
                        affine,
                        &brush_from_fill(&fill, bbox),
                        None,
                        &rect,
                    );
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
            Object::Line(_) => {
                let x1 = state["x1"].as_f64().unwrap_or(0.0);
                let y1 = state["y1"].as_f64().unwrap_or(0.0);
                let x2 = state["x2"].as_f64().unwrap_or(0.0);
                let y2 = state["y2"].as_f64().unwrap_or(0.0);
                let sw = state["stroke_width"].as_f64().unwrap_or(1.0);
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;
                let stroke_color =
                    parse_vello_color(state["stroke"].as_str().unwrap_or("#FFFFFF"), opacity);

                let line = Line::new((x1, y1), (x2, y2));
                let mut stroke = flat_stroke(sw);
                if let Some(frac) = state["draw_fraction"].as_f64() {
                    // Same partial-reveal dash the CPU backend uses.
                    let dx = (x2 - x1) as f32;
                    let dy = (y2 - y1) as f32;
                    let length = (dx * dx + dy * dy).sqrt().max(0.001);
                    let dashes = crate::common::stroke::draw_fraction_dash(frac as f32, length);
                    stroke = stroke.with_dashes(0.0, dashes.iter().map(|d| *d as f64));
                }
                scene.stroke(&stroke, affine, stroke_color, None, &line);
            }
            Object::Arrow(_) => {
                let from = state["from"].as_array();
                let to = state["to"].as_array();
                let (from, to) = match (from, to) {
                    (Some(f), Some(t)) if f.len() >= 2 && t.len() >= 2 => (f, t),
                    _ => return,
                };
                let fx = from[0].as_f64().unwrap_or(0.0);
                let fy = from[1].as_f64().unwrap_or(0.0);
                let tx = to[0].as_f64().unwrap_or(0.0);
                let ty = to[1].as_f64().unwrap_or(0.0);
                let sw = state["stroke_width"].as_f64().unwrap_or(1.0);
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;
                let color =
                    parse_vello_color(state["color"].as_str().unwrap_or("#FFFFFF"), opacity);

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
            Object::BezierCurve(_) => {
                let get_pt = |key: &str| -> Option<(f64, f64)> {
                    let arr = state[key].as_array()?;
                    Some((arr.first()?.as_f64()?, arr.get(1)?.as_f64()?))
                };
                let (x0, y0) = match get_pt("p0") {
                    Some(p) => p,
                    None => return,
                };
                let (x1, y1) = match get_pt("p1") {
                    Some(p) => p,
                    None => return,
                };
                let (x2, y2) = match get_pt("p2") {
                    Some(p) => p,
                    None => return,
                };
                let (x3, y3) = match get_pt("p3") {
                    Some(p) => p,
                    None => return,
                };
                let sw = state["stroke_width"].as_f64().unwrap_or(1.0);
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;
                let stroke_color =
                    parse_vello_color(state["stroke"].as_str().unwrap_or("#FFFFFF"), opacity);
                let draw_fraction = state["draw_fraction"].as_f64().unwrap_or(1.0);

                let t = draw_fraction.clamp(0.0, 1.0);
                let lerp = |a: f64, b: f64| a + (b - a) * t;
                let ax = lerp(x0, x1);
                let ay = lerp(y0, y1);
                let bx = lerp(x1, x2);
                let by_ = lerp(y1, y2);
                let cx_ = lerp(x2, x3);
                let cy_ = lerp(y2, y3);
                let dx = lerp(ax, bx);
                let dy = lerp(ay, by_);
                let ex = lerp(bx, cx_);
                let ey = lerp(by_, cy_);
                let fx = lerp(dx, ex);
                let fy = lerp(dy, ey);

                let mut path = BezPath::new();
                path.move_to((x0, y0));
                path.curve_to((ax, ay), (dx, dy), (fx, fy));
                scene.stroke(&flat_stroke(sw), affine, stroke_color, None, &path);
            }
            Object::Polygon(_) => {
                let points = match state["points"].as_array() {
                    Some(p) => p,
                    None => return,
                };
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;

                let mut path = BezPath::new();
                let (mut min_x, mut min_y) = (f32::INFINITY, f32::INFINITY);
                let (mut max_x, mut max_y) = (f32::NEG_INFINITY, f32::NEG_INFINITY);
                for (i, p) in points.iter().enumerate() {
                    let arr = match p.as_array() {
                        Some(a) => a,
                        None => continue,
                    };
                    let x = arr.first().and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let y = arr.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0);
                    min_x = min_x.min(x as f32);
                    min_y = min_y.min(y as f32);
                    max_x = max_x.max(x as f32);
                    max_y = max_y.max(y as f32);
                    if i == 0 {
                        path.move_to((x, y));
                    } else {
                        path.line_to((x, y));
                    }
                }
                path.close_path();
                if !min_x.is_finite() {
                    return;
                }
                let bbox = (min_x, min_y, max_x - min_x, max_y - min_y);

                let fill = crate::common::fill::parse_fill(&state["fill"], opacity)
                    .unwrap_or_else(|| crate::common::fill::FillSpec::solid("#FFFFFF", opacity));
                scene.fill(
                    Fill::NonZero,
                    affine,
                    &brush_from_fill(&fill, bbox),
                    None,
                    &path,
                );

                if let Some(stroke) = crate::common::fill::parse_stroke(state, opacity) {
                    let sw = state["stroke_width"].as_f64().unwrap_or(1.0);
                    scene.stroke(
                        &flat_stroke(sw),
                        affine,
                        &brush_from_fill(&stroke, bbox),
                        None,
                        &path,
                    );
                }
            }
            Object::Path(_) => {
                let d = state["d"].as_str().unwrap_or("");
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;

                if let Some(data) = crate::common::path::parse_svg_path(d) {
                    let path = crate::common::path::to_kurbo_path(&data);
                    let bbox = crate::common::path::bbox(&data).unwrap_or((0.0, 0.0, 0.0, 0.0));
                    if let Some(fill) = crate::common::fill::parse_fill(&state["fill"], opacity) {
                        scene.fill(
                            Fill::NonZero,
                            affine,
                            &brush_from_fill(&fill, bbox),
                            None,
                            &path,
                        );
                    }
                    if let Some(stroke) = crate::common::fill::parse_stroke(state, opacity) {
                        let sw = state["stroke_width"].as_f64().unwrap_or(1.0);
                        scene.stroke(
                            &flat_stroke(sw),
                            affine,
                            &brush_from_fill(&stroke, bbox),
                            None,
                            &path,
                        );
                    }
                }
            }
            Object::NumberLine(_) => {
                let start = state["start"].as_f64().unwrap_or(0.0);
                let end = state["end"].as_f64().unwrap_or(10.0);
                let step = state["step"].as_f64().unwrap_or(1.0);
                let x = state["x"].as_f64().unwrap_or(0.0);
                let y = state["y"].as_f64().unwrap_or(0.0);
                let length = state["length"].as_f64().unwrap_or(400.0);
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;
                let color =
                    parse_vello_color(state["color"].as_str().unwrap_or("#FFFFFF"), opacity);
                let range = end - start;
                if range <= 0.0 || step <= 0.0 {
                    return;
                }

                let stroke = flat_stroke(2.0);
                let main = Line::new((x, y), (x + length, y));
                scene.stroke(&stroke, affine, color, None, &main);

                let tick_h = 8.0;
                let mut t = start;
                while t <= end + 1e-4 {
                    let px = x + (t - start) / range * length;
                    let tick = Line::new((px, y - tick_h), (px, y + tick_h));
                    scene.stroke(&stroke, affine, color, None, &tick);
                    t += step;
                }
            }
            Object::Axes(_) => {
                let x = state["x"].as_f64().unwrap_or(0.0);
                let y = state["y"].as_f64().unwrap_or(0.0);
                let x_range = state["x_range"].as_array();
                let y_range = state["y_range"].as_array();
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;
                let color =
                    parse_vello_color(state["color"].as_str().unwrap_or("#FFFFFF"), opacity);
                let scale = state["scale"].as_f64().unwrap_or(40.0);
                let x_step = state["x_step"].as_f64().unwrap_or(1.0);
                let y_step = state["y_step"].as_f64().unwrap_or(1.0);
                let draw_grid = state["grid"].as_bool().unwrap_or(false);

                let x_min = x_range
                    .and_then(|r| r.first())
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let x_max = x_range
                    .and_then(|r| r.get(1))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(10.0);
                let y_min = y_range
                    .and_then(|r| r.first())
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let y_max = y_range
                    .and_then(|r| r.get(1))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(10.0);

                let ox = x + (0.0 - x_min) * scale;
                let oy = y - (0.0 - y_min) * scale;

                let axis_stroke = flat_stroke(2.0);
                let tick_stroke = flat_stroke(1.0);
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
                let x_count = ((x_max - x_min) / x_step).ceil() as i32;
                for i in 0..=x_count {
                    let tx = x_min + i as f64 * x_step;
                    if tx > x_max + 1e-4 {
                        break;
                    }
                    let px = ox + tx * scale;
                    scene.stroke(
                        &tick_stroke,
                        affine,
                        color,
                        None,
                        &Line::new((px, oy - 5.0), (px, oy + 5.0)),
                    );
                    if draw_grid && (tx - 0.0).abs() > 1e-4 {
                        scene.stroke(
                            &tick_stroke,
                            affine,
                            grid_color,
                            None,
                            &Line::new((px, oy - y_min * scale), (px, oy - y_max * scale)),
                        );
                    }
                }

                // Y ticks and optional horizontal grid lines
                let y_count = ((y_max - y_min) / y_step).ceil() as i32;
                for i in 0..=y_count {
                    let ty = y_min + i as f64 * y_step;
                    if ty > y_max + 1e-4 {
                        break;
                    }
                    let py = oy - ty * scale;
                    scene.stroke(
                        &tick_stroke,
                        affine,
                        color,
                        None,
                        &Line::new((ox - 5.0, py), (ox + 5.0, py)),
                    );
                    if draw_grid && (ty - 0.0).abs() > 1e-4 {
                        scene.stroke(
                            &tick_stroke,
                            affine,
                            grid_color,
                            None,
                            &Line::new((ox + x_min * scale, py), (ox + x_max * scale, py)),
                        );
                    }
                }
            }
            Object::Plot(props) => {
                let axes_id = state["axes_id"].as_str().unwrap_or(&props.axes_id);
                let function_str = state["function_str"]
                    .as_str()
                    .unwrap_or(&props.function_str);
                let sw = state["stroke_width"].as_f64().unwrap_or(2.0);
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;
                let color =
                    parse_vello_color(state["color"].as_str().unwrap_or("#FFFFFF"), opacity);
                let total_samples = state["sample_count"].as_u64().unwrap_or(200) as usize;
                let draw_fraction = state["draw_fraction"].as_f64().unwrap_or(1.0) as f32;
                let samples = ((total_samples as f32 * draw_fraction) as usize).max(1);

                let axes_s = match states.get(axes_id) {
                    Some(s) => s,
                    None => return,
                };
                let x = axes_s["x"].as_f64().unwrap_or(0.0);
                let y = axes_s["y"].as_f64().unwrap_or(0.0);
                let x_arr = axes_s["x_range"].as_array();
                let y_arr = axes_s["y_range"].as_array();
                let x_min = x_arr
                    .and_then(|a| a.first())
                    .and_then(|v| v.as_f64())
                    .unwrap_or(-10.0);
                let x_max = x_arr
                    .and_then(|a| a.get(1))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(10.0);
                let y_min = y_arr
                    .and_then(|a| a.first())
                    .and_then(|v| v.as_f64())
                    .unwrap_or(-10.0);
                let y_max = y_arr
                    .and_then(|a| a.get(1))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(10.0);
                let scale = axes_s["scale"].as_f64().unwrap_or(40.0);
                let ox = x + (0.0 - x_min) * scale;
                let oy = y - (0.0 - y_min) * scale;
                let x_end = x_min + (x_max - x_min) * draw_fraction as f64;

                // Normalize bare math functions to evalexpr math:: namespace
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

                let mut path = BezPath::new();
                let mut started = false;
                for i in 0..=samples {
                    let mx = x_min + (i as f64 / samples as f64) * (x_end - x_min);
                    let eval_ctx = evalexpr::context_map! { "x" => mx };
                    let my = match eval_ctx
                        .and_then(|c| evalexpr::eval_number_with_context(function_str, &c))
                    {
                        Ok(v) => v,
                        Err(_) => {
                            started = false;
                            continue;
                        }
                    };
                    let y_margin = (y_max - y_min).abs();
                    if !my.is_finite() || my < y_min - y_margin || my > y_max + y_margin {
                        started = false;
                        continue;
                    }
                    let sx = ox + mx * scale;
                    let sy = oy - my * scale;
                    if !started {
                        path.move_to((sx, sy));
                        started = true;
                    } else {
                        path.line_to((sx, sy));
                    }
                }
                if !path.is_empty() {
                    scene.stroke(&KurboStroke::new(sw), affine, color, None, &path);
                }
            }
            Object::Text(_) | Object::LaTeX(_) | Object::MathML(_) => {
                let x = state["x"].as_f64().unwrap_or(0.0) as f32;
                let y = state["y"].as_f64().unwrap_or(0.0) as f32;
                let font_size = state["font_size"].as_f64().unwrap_or(24.0) as f32;
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;
                let color_str = state["color"].as_str().unwrap_or("#FFFFFF");
                let font_id = state["font_id"].as_str();
                let align = state["align"].as_str().unwrap_or("left");
                let letter_spacing = state["letter_spacing"].as_f64().unwrap_or(0.0) as f32;

                // Resolve the displayed string: raw content for Text, Unicode-
                // converted (and optionally write-on-clipped) for LaTeX, tag-
                // stripped for MathML — matching the Skia backend exactly.
                let rendered: String = match obj {
                    Object::Text(_) => state["content"].as_str().unwrap_or("").to_string(),
                    Object::LaTeX(_) => {
                        let mut s = crate::skia_backend::latex_to_unicode(
                            state["expression"].as_str().unwrap_or(""),
                        );
                        if let Some(frac) = state["draw_fraction"].as_f64() {
                            let frac = (frac as f32).clamp(0.0, 1.0);
                            let visible = (s.chars().count() as f32 * frac).floor() as usize;
                            s = s.chars().take(visible).collect();
                        }
                        s
                    }
                    _ => crate::skia_backend::mathml_to_unicode(
                        state["markup"].as_str().unwrap_or(""),
                    ),
                };

                if let Some(tb) = raster::rasterize_text(
                    &self.text_engine,
                    &rendered,
                    font_size,
                    color_str,
                    font_id,
                    align,
                    letter_spacing,
                    opacity,
                ) {
                    let img = rgba_to_image(tb.rgba, tb.width, tb.height);
                    let t = affine
                        * Affine::translate(Vec2::new(
                            (x + tb.place_x) as f64,
                            (y + tb.place_y) as f64,
                        ));
                    scene.draw_image(&img, t);
                }
            }
            Object::Image(_) | Object::SVG(_) => {
                let asset_id = state["asset_id"].as_str().unwrap_or("");
                if asset_id.is_empty() {
                    return;
                }
                let x = state["x"].as_f64().unwrap_or(0.0);
                let y = state["y"].as_f64().unwrap_or(0.0);
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;
                let rotation = state["rotation"].as_f64().unwrap_or(0.0) as f32;
                let want_w = state["width"].as_f64().map(|v| v as f32);
                let want_h = state["height"].as_f64().map(|v| v as f32);
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
            Object::Particles(_) => {
                let count = state["count"].as_u64().unwrap_or(0) as u32;
                if count == 0 {
                    return;
                }
                let ex = state["emitter_x"].as_f64().unwrap_or(0.0) as f32;
                let ey = state["emitter_y"].as_f64().unwrap_or(0.0) as f32;
                let lifetime = state["lifetime"].as_f64().unwrap_or(2.0) as f32;
                let speed = state["speed"].as_f64().unwrap_or(120.0) as f32;
                let spread = state["spread"].as_f64().unwrap_or(360.0) as f32;
                let size = state["size"].as_f64().unwrap_or(3.0) as f32;
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;
                let color_hex = state["color"].as_str().unwrap_or("#FFFFFF");
                let base = parse_vello_color(color_hex, 1.0);
                for dot in raster::simulate_particles(
                    count,
                    ex,
                    ey,
                    lifetime,
                    speed,
                    spread,
                    size,
                    opacity,
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
    }
}

impl Renderer for VelloRenderer {
    fn render_frame(
        &mut self,
        objects: &HashMap<String, Object>,
        states: &HashMap<String, Value>,
        width: u32,
        height: u32,
        background: &str,
        camera: Option<&CameraState>,
    ) -> Result<Vec<u8>, RendererError> {
        let scene = self.build_scene(objects, states, width, height, background, camera);

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
