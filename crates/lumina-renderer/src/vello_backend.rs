use crate::{Renderer, RendererError};
use lumina_schema::{CameraState, Object};
use serde_json::Value;
use std::collections::HashMap;
use std::num::NonZeroUsize;

// Use vello's re-exported wgpu (v0.20) — workspace wgpu is v22, types are incompatible
use vello::wgpu;
use vello::{
    AaConfig, AaSupport, RenderParams, RendererOptions, Scene,
    kurbo::{Affine, BezPath, Circle, Line, Rect, Stroke as KurboStroke, Vec2},
    peniko::{Color, Fill},
};

pub struct VelloRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: vello::Renderer,
}

impl VelloRenderer {
    pub async fn new() -> Result<Self, RendererError> {
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

        Ok(Self { device, queue, renderer })
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

        // Camera root transform
        let root_affine = if let Some(cam) = camera {
            let cx = width as f64 / 2.0;
            let cy = height as f64 / 2.0;
            Affine::translate(Vec2::new(cx + cam.x as f64, cy + cam.y as f64))
                * Affine::scale(cam.zoom as f64)
                * Affine::translate(Vec2::new(-cx, -cy))
        } else {
            Affine::IDENTITY
        };

        // Determine which objects are group children (exclude from root render pass)
        let mut child_ids = std::collections::HashSet::new();
        for obj in objects.values() {
            if let Object::Group(g) = obj {
                for cid in &g.children {
                    child_ids.insert(cid.clone());
                }
            }
        }

        // Sort root objects by z-index
        let mut roots: Vec<(&str, i32)> = objects
            .iter()
            .filter(|(id, _)| !child_ids.contains(*id))
            .map(|(id, obj)| (id.as_str(), z_index_of(obj)))
            .collect();
        roots.sort_by_key(|(_, z)| *z);

        for (id, _) in roots {
            self.draw_node(&mut scene, id, objects, states, root_affine);
        }

        scene
    }

    fn draw_node(
        &self,
        scene: &mut Scene,
        id: &str,
        objects: &HashMap<String, Object>,
        states: &HashMap<String, Value>,
        parent_affine: Affine,
    ) {
        let obj = match objects.get(id) { Some(o) => o, None => return };
        let state = match states.get(id) { Some(s) => s, None => return };

        match obj {
            Object::Group(props) => {
                let x = state["x"].as_f64().unwrap_or(0.0);
                let y = state["y"].as_f64().unwrap_or(0.0);
                let scale = state["scale"].as_f64().unwrap_or(1.0);
                let rotation_deg = state["rotation"].as_f64().unwrap_or(0.0);

                let mut affine = parent_affine * Affine::translate(Vec2::new(x, y));
                if scale != 1.0 {
                    affine = affine * Affine::scale(scale);
                }
                if rotation_deg != 0.0 {
                    affine = affine * Affine::rotate(rotation_deg.to_radians());
                }

                let mut children: Vec<(&str, i32)> = props.children.iter()
                    .map(|cid| (cid.as_str(), objects.get(cid).map(z_index_of).unwrap_or(0)))
                    .collect();
                children.sort_by_key(|(_, z)| *z);

                for (child_id, _) in children {
                    self.draw_node(scene, child_id, objects, states, affine);
                }
            }
            _ => self.draw_leaf(scene, obj, state, parent_affine, objects, states),
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
                if radius <= 0.0 { return; }
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;

                let circle = Circle::new((cx, cy), radius);
                let fill_color = parse_vello_color(state["fill"].as_str().unwrap_or("#FFFFFF"), opacity);
                scene.fill(Fill::NonZero, affine, fill_color, None, &circle);

                if let Some(stroke_hex) = state["stroke"].as_str() {
                    let sw = state["stroke_width"].as_f64().unwrap_or(1.0);
                    let stroke_color = parse_vello_color(stroke_hex, opacity);
                    scene.stroke(&KurboStroke::new(sw), affine, stroke_color, None, &circle);
                }
            }
            Object::Rectangle(_) => {
                let x = state["x"].as_f64().unwrap_or(0.0);
                let y = state["y"].as_f64().unwrap_or(0.0);
                let w = state["width"].as_f64().unwrap_or(0.0);
                let h = state["height"].as_f64().unwrap_or(0.0);
                if w <= 0.0 || h <= 0.0 { return; }
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;

                let rect = Rect::new(x, y, x + w, y + h);
                let fill_color = parse_vello_color(state["fill"].as_str().unwrap_or("#FFFFFF"), opacity);
                scene.fill(Fill::NonZero, affine, fill_color, None, &rect);

                if let Some(stroke_hex) = state["stroke"].as_str() {
                    let sw = state["stroke_width"].as_f64().unwrap_or(1.0);
                    let stroke_color = parse_vello_color(stroke_hex, opacity);
                    scene.stroke(&KurboStroke::new(sw), affine, stroke_color, None, &rect);
                }
            }
            Object::Line(_) => {
                let x1 = state["x1"].as_f64().unwrap_or(0.0);
                let y1 = state["y1"].as_f64().unwrap_or(0.0);
                let x2 = state["x2"].as_f64().unwrap_or(0.0);
                let y2 = state["y2"].as_f64().unwrap_or(0.0);
                let sw = state["stroke_width"].as_f64().unwrap_or(1.0);
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;
                let stroke_color = parse_vello_color(state["stroke"].as_str().unwrap_or("#FFFFFF"), opacity);

                let draw_fraction = state["draw_fraction"].as_f64().unwrap_or(1.0) as f32;
                let tx = x1 + (x2 - x1) * draw_fraction as f64;
                let ty = y1 + (y2 - y1) * draw_fraction as f64;

                let line = Line::new((x1, y1), (tx, ty));
                scene.stroke(&KurboStroke::new(sw), affine, stroke_color, None, &line);
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
                let color = parse_vello_color(state["color"].as_str().unwrap_or("#FFFFFF"), opacity);

                let line = Line::new((fx, fy), (tx, ty));
                scene.stroke(&KurboStroke::new(sw), affine, color, None, &line);

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
                scene.stroke(&KurboStroke::new(sw), affine, color, None, &head);
            }
            Object::BezierCurve(_) => {
                let get_pt = |key: &str| -> Option<(f64, f64)> {
                    let arr = state[key].as_array()?;
                    Some((arr.get(0)?.as_f64()?, arr.get(1)?.as_f64()?))
                };
                let (x0, y0) = match get_pt("p0") { Some(p) => p, None => return };
                let (x1, y1) = match get_pt("p1") { Some(p) => p, None => return };
                let (x2, y2) = match get_pt("p2") { Some(p) => p, None => return };
                let (x3, y3) = match get_pt("p3") { Some(p) => p, None => return };
                let sw = state["stroke_width"].as_f64().unwrap_or(1.0);
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;
                let stroke_color = parse_vello_color(state["stroke"].as_str().unwrap_or("#FFFFFF"), opacity);
                let draw_fraction = state["draw_fraction"].as_f64().unwrap_or(1.0);

                let t = draw_fraction.clamp(0.0, 1.0);
                let lerp = |a: f64, b: f64| a + (b - a) * t;
                let ax = lerp(x0, x1); let ay = lerp(y0, y1);
                let bx = lerp(x1, x2); let by_ = lerp(y1, y2);
                let cx_ = lerp(x2, x3); let cy_ = lerp(y2, y3);
                let dx = lerp(ax, bx); let dy = lerp(ay, by_);
                let ex = lerp(bx, cx_); let ey = lerp(by_, cy_);
                let fx = lerp(dx, ex); let fy = lerp(dy, ey);

                let mut path = BezPath::new();
                path.move_to((x0, y0));
                path.curve_to((ax, ay), (dx, dy), (fx, fy));
                scene.stroke(&KurboStroke::new(sw), affine, stroke_color, None, &path);
            }
            Object::Polygon(_) => {
                let points = match state["points"].as_array() { Some(p) => p, None => return };
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;

                let mut path = BezPath::new();
                for (i, p) in points.iter().enumerate() {
                    let arr = match p.as_array() { Some(a) => a, None => continue };
                    let x = arr.get(0).and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let y = arr.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0);
                    if i == 0 { path.move_to((x, y)); } else { path.line_to((x, y)); }
                }
                path.close_path();

                let fill_color = parse_vello_color(state["fill"].as_str().unwrap_or("#FFFFFF"), opacity);
                scene.fill(Fill::NonZero, affine, fill_color, None, &path);

                if let Some(stroke_hex) = state["stroke"].as_str() {
                    let sw = state["stroke_width"].as_f64().unwrap_or(1.0);
                    let stroke_color = parse_vello_color(stroke_hex, opacity);
                    scene.stroke(&KurboStroke::new(sw), affine, stroke_color, None, &path);
                }
            }
            Object::Path(_) => {
                let d = state["d"].as_str().unwrap_or("");
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;

                if let Some(path) = parse_svg_path_kurbo(d) {
                    if let Some(fill_hex) = state["fill"].as_str() {
                        let fill_color = parse_vello_color(fill_hex, opacity);
                        scene.fill(Fill::NonZero, affine, fill_color, None, &path);
                    }
                    if let Some(stroke_hex) = state["stroke"].as_str() {
                        let sw = state["stroke_width"].as_f64().unwrap_or(1.0);
                        let stroke_color = parse_vello_color(stroke_hex, opacity);
                        scene.stroke(&KurboStroke::new(sw), affine, stroke_color, None, &path);
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
                let color = parse_vello_color(state["color"].as_str().unwrap_or("#FFFFFF"), opacity);
                let range = end - start;
                if range <= 0.0 || step <= 0.0 { return; }

                let stroke = KurboStroke::new(2.0);
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
                let color = parse_vello_color(state["color"].as_str().unwrap_or("#FFFFFF"), opacity);
                let scale = state["scale"].as_f64().unwrap_or(40.0);
                let x_step = state["x_step"].as_f64().unwrap_or(1.0);
                let y_step = state["y_step"].as_f64().unwrap_or(1.0);
                let draw_grid = state["grid"].as_bool().unwrap_or(false);

                let x_min = x_range.and_then(|r| r.get(0)).and_then(|v| v.as_f64()).unwrap_or(0.0);
                let x_max = x_range.and_then(|r| r.get(1)).and_then(|v| v.as_f64()).unwrap_or(10.0);
                let y_min = y_range.and_then(|r| r.get(0)).and_then(|v| v.as_f64()).unwrap_or(0.0);
                let y_max = y_range.and_then(|r| r.get(1)).and_then(|v| v.as_f64()).unwrap_or(10.0);

                let ox = x + (0.0 - x_min) * scale;
                let oy = y - (0.0 - y_min) * scale;

                let axis_stroke = KurboStroke::new(2.0);
                let tick_stroke = KurboStroke::new(1.0);
                let grid_color = Color::rgba8(
                    color.r, color.g, color.b,
                    ((color.a as f32) * 0.2) as u8,
                );

                // X axis
                scene.stroke(&axis_stroke, affine, color, None,
                    &Line::new((ox + x_min * scale, oy), (ox + x_max * scale, oy)));
                // Y axis
                scene.stroke(&axis_stroke, affine, color, None,
                    &Line::new((ox, oy - y_min * scale), (ox, oy - y_max * scale)));

                // X ticks and optional vertical grid lines
                let x_count = ((x_max - x_min) / x_step).ceil() as i32;
                for i in 0..=x_count {
                    let tx = x_min + i as f64 * x_step;
                    if tx > x_max + 1e-4 { break; }
                    let px = ox + tx * scale;
                    scene.stroke(&tick_stroke, affine, color, None,
                        &Line::new((px, oy - 5.0), (px, oy + 5.0)));
                    if draw_grid && (tx - 0.0).abs() > 1e-4 {
                        scene.stroke(&tick_stroke, affine, grid_color, None,
                            &Line::new((px, oy - y_min * scale), (px, oy - y_max * scale)));
                    }
                }

                // Y ticks and optional horizontal grid lines
                let y_count = ((y_max - y_min) / y_step).ceil() as i32;
                for i in 0..=y_count {
                    let ty = y_min + i as f64 * y_step;
                    if ty > y_max + 1e-4 { break; }
                    let py = oy - ty * scale;
                    scene.stroke(&tick_stroke, affine, color, None,
                        &Line::new((ox - 5.0, py), (ox + 5.0, py)));
                    if draw_grid && (ty - 0.0).abs() > 1e-4 {
                        scene.stroke(&tick_stroke, affine, grid_color, None,
                            &Line::new((ox + x_min * scale, py), (ox + x_max * scale, py)));
                    }
                }
            }
            Object::Plot(props) => {
                let axes_id = state["axes_id"].as_str().unwrap_or(&props.axes_id);
                let function_str = state["function_str"].as_str().unwrap_or(&props.function_str);
                let sw = state["stroke_width"].as_f64().unwrap_or(2.0);
                let opacity = state["opacity"].as_f64().unwrap_or(1.0) as f32;
                let color = parse_vello_color(state["color"].as_str().unwrap_or("#FFFFFF"), opacity);
                let total_samples = state["sample_count"].as_u64().unwrap_or(200) as usize;
                let draw_fraction = state["draw_fraction"].as_f64().unwrap_or(1.0) as f32;
                let samples = ((total_samples as f32 * draw_fraction) as usize).max(1);

                let axes_s = match states.get(axes_id) { Some(s) => s, None => return };
                let x = axes_s["x"].as_f64().unwrap_or(0.0);
                let y = axes_s["y"].as_f64().unwrap_or(0.0);
                let x_arr = axes_s["x_range"].as_array();
                let y_arr = axes_s["y_range"].as_array();
                let x_min = x_arr.and_then(|a| a.get(0)).and_then(|v| v.as_f64()).unwrap_or(-10.0);
                let x_max = x_arr.and_then(|a| a.get(1)).and_then(|v| v.as_f64()).unwrap_or(10.0);
                let y_min = y_arr.and_then(|a| a.get(0)).and_then(|v| v.as_f64()).unwrap_or(-10.0);
                let y_max = y_arr.and_then(|a| a.get(1)).and_then(|v| v.as_f64()).unwrap_or(10.0);
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
                    let my = match eval_ctx.and_then(|c| evalexpr::eval_number_with_context(function_str, &c)) {
                        Ok(v) => v,
                        Err(_) => { started = false; continue; }
                    };
                    let y_margin = (y_max - y_min).abs();
                    if !my.is_finite() || my < y_min - y_margin || my > y_max + y_margin {
                        started = false; continue;
                    }
                    let sx = ox + mx * scale;
                    let sy = oy - my * scale;
                    if !started { path.move_to((sx, sy)); started = true; } else { path.line_to((sx, sy)); }
                }
                if !path.is_empty() {
                    scene.stroke(&KurboStroke::new(sw), affine, color, None, &path);
                }
            }
            // Text and LaTeX require font shaping — not yet implemented in GPU backend
            Object::Text(_) | Object::LaTeX(_) => {}
            // Image/SVG require asset loading
            Object::Image(_) | Object::SVG(_) => {}
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
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
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
        let padded_bytes_per_row = (unpadded_bytes_per_row + align - 1) / align * align;
        let buffer_size = (padded_bytes_per_row * height) as u64;

        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("vello_staging"),
            size: buffer_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
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
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
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

    fn load_font(&mut self, _id: &str, _data: &[u8]) -> Result<(), RendererError> {
        // Font loading for GPU text rendering requires skrifa integration — not yet implemented
        Ok(())
    }
}

fn z_index_of(obj: &Object) -> i32 {
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

fn parse_vello_color(hex: &str, opacity: f32) -> Color {
    let hex = hex.trim_start_matches('#');
    let (r, g, b, a_factor) = match hex.len() {
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255);
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(255);
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(255);
            (r, g, b, 1.0_f32)
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255);
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(255);
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(255);
            let a = u8::from_str_radix(&hex[6..8], 16).unwrap_or(255) as f32 / 255.0;
            (r, g, b, a)
        }
        3 => {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).unwrap_or(255);
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).unwrap_or(255);
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).unwrap_or(255);
            (r, g, b, 1.0_f32)
        }
        _ => (255, 255, 255, 1.0_f32),
    };
    Color::rgba8(r, g, b, ((opacity * a_factor) * 255.0) as u8)
}

fn parse_svg_path_kurbo(d: &str) -> Option<BezPath> {
    let mut path = BezPath::new();
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
    let mut cur_x = 0.0_f64;
    let mut cur_y = 0.0_f64;
    let mut has_content = false;

    macro_rules! next_f64 {
        () => {{
            i += 1;
            tokens.get(i - 1).and_then(|s| s.parse::<f64>().ok())?
        }};
    }

    while i < tokens.len() {
        let cmd = tokens[i];
        i += 1;
        match cmd {
            "M" => {
                let x = next_f64!(); let y = next_f64!();
                path.move_to((x, y));
                cur_x = x; cur_y = y; has_content = true;
            }
            "m" => {
                let dx = next_f64!(); let dy = next_f64!();
                path.move_to((cur_x + dx, cur_y + dy));
                cur_x += dx; cur_y += dy; has_content = true;
            }
            "L" => {
                let x = next_f64!(); let y = next_f64!();
                path.line_to((x, y));
                cur_x = x; cur_y = y;
            }
            "l" => {
                let dx = next_f64!(); let dy = next_f64!();
                path.line_to((cur_x + dx, cur_y + dy));
                cur_x += dx; cur_y += dy;
            }
            "H" => { let x = next_f64!(); path.line_to((x, cur_y)); cur_x = x; }
            "h" => { let dx = next_f64!(); path.line_to((cur_x + dx, cur_y)); cur_x += dx; }
            "V" => { let y = next_f64!(); path.line_to((cur_x, y)); cur_y = y; }
            "v" => { let dy = next_f64!(); path.line_to((cur_x, cur_y + dy)); cur_y += dy; }
            "C" => {
                let x1 = next_f64!(); let y1 = next_f64!();
                let x2 = next_f64!(); let y2 = next_f64!();
                let x = next_f64!(); let y = next_f64!();
                path.curve_to((x1, y1), (x2, y2), (x, y));
                cur_x = x; cur_y = y;
            }
            "c" => {
                let dx1 = next_f64!(); let dy1 = next_f64!();
                let dx2 = next_f64!(); let dy2 = next_f64!();
                let dx = next_f64!(); let dy = next_f64!();
                path.curve_to((cur_x + dx1, cur_y + dy1), (cur_x + dx2, cur_y + dy2), (cur_x + dx, cur_y + dy));
                cur_x += dx; cur_y += dy;
            }
            "Z" | "z" => { path.close_path(); }
            _ => {}
        }
    }

    if has_content { Some(path) } else { None }
}
