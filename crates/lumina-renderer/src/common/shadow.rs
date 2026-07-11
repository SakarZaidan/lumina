//! Shared drop-shadow pipeline: parse → silhouette → separable box blur.
//!
//! The blur operates on premultiplied RGBA bytes in a tiny-skia `Pixmap`,
//! which both backends can produce: the Skia backend composites the result
//! directly, the Vello backend converts it to a straight-alpha
//! `peniko::Image` and draws it into the GPU scene. One blur implementation
//! means shadows are near-identical across backends by construction.

use serde_json::Value;
use tiny_skia::{BlendMode, FillRule, FilterQuality, Paint, Path, Pixmap, PixmapPaint, Transform};

/// A resolved drop-shadow specification (straight-alpha RGBA color).
pub(crate) struct ShadowSpec {
    pub color: [u8; 4],
    pub blur: f32,
    pub dx: f32,
    pub dy: f32,
    pub opacity: f32,
}

pub(crate) fn parse_shadow(state: &Value) -> Option<ShadowSpec> {
    let map = match state.get("shadow") {
        Some(Value::Object(m)) => m,
        _ => return None,
    };
    let color_hex = map
        .get("color")
        .and_then(|c| c.as_str())
        .unwrap_or("#000000");
    Some(ShadowSpec {
        color: super::color::parse_rgba8(color_hex, 1.0),
        blur: map.get("blur").and_then(|b| b.as_f64()).unwrap_or(0.0) as f32,
        dx: map.get("dx").and_then(|d| d.as_f64()).unwrap_or(0.0) as f32,
        dy: map.get("dy").and_then(|d| d.as_f64()).unwrap_or(0.0) as f32,
        opacity: map.get("opacity").and_then(|o| o.as_f64()).unwrap_or(1.0) as f32,
    })
}

/// Rasterize the blurred, offset silhouette of `path` into its own pixmap
/// (canvas-sized, premultiplied). Returns `None` when allocation fails.
pub(crate) fn shadow_pixmap(
    width: u32,
    height: u32,
    path: &Path,
    transform: Transform,
    shadow: &ShadowSpec,
) -> Option<Pixmap> {
    let mut off = Pixmap::new(width, height)?;
    let mut paint = Paint::default();
    paint.set_color(super::color::to_tiny(shadow.color));
    paint.anti_alias = true;
    let t = Transform::from_translate(shadow.dx, shadow.dy).pre_concat(transform);
    off.fill_path(path, &paint, FillRule::Winding, t, None);
    let r = shadow.blur.clamp(0.0, 50.0).round() as usize;
    box_blur(&mut off, r);
    Some(off)
}

/// Render a blurred, offset silhouette of `path` beneath a shape.
pub(crate) fn draw_shadow(
    dst: &mut Pixmap,
    path: &Path,
    transform: Transform,
    shadow: &ShadowSpec,
) {
    let off = match shadow_pixmap(dst.width(), dst.height(), path, transform, shadow) {
        Some(p) => p,
        None => return,
    };
    let pp = PixmapPaint {
        opacity: shadow.opacity.clamp(0.0, 1.0),
        blend_mode: BlendMode::SourceOver,
        quality: FilterQuality::Nearest,
    };
    dst.draw_pixmap(0, 0, off.as_ref(), &pp, Transform::identity(), None);
}

/// Separable 3-pass box blur (≈ Gaussian) over premultiplied RGBA bytes.
pub(crate) fn box_blur(pm: &mut Pixmap, radius: usize) {
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
