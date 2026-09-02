//! Shared CPU rasterization helpers used by both backends.
//!
//! The GPU (Vello) backend reuses the proven fontdue / tiny-skia raster path
//! for text, math and assets by producing straight-alpha RGBA buffers that are
//! wrapped in a `peniko::Image` and composited via `Scene::draw_image`. This
//! avoids maintaining a second (skrifa) font stack while reaching parity with
//! the Skia backend. Particle simulation is shared so both backends emit
//! byte-identical, scrub-reproducible output.

use lumina_text::TextEngine;
use tiny_skia::{Color, ColorU8, Pixmap};

/// A standalone rasterized text run. `rgba` is straight-alpha (non-premultiplied)
/// RGBA8 suitable for `peniko::Image`. `place_x`/`place_y` are screen-space
/// offsets from the text anchor `(x, y)` to the bitmap's top-left, so drawing
/// the bitmap at `(anchor_x + place_x, anchor_y + place_y)` reproduces the
/// baseline-anchored placement the Skia backend uses.
pub(crate) struct TextBitmap {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub place_x: f32,
    pub place_y: f32,
}

/// Rasterize a text run with per-character font fallback, alignment and
/// letter-spacing into a tight standalone bitmap. Returns `None` for empty or
/// zero-area runs (e.g. all whitespace clipped by `draw_fraction`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn rasterize_text(
    engine: &TextEngine,
    content: &str,
    font_size: f32,
    color_str: &str,
    font_id: Option<&str>,
    align: &str,
    letter_spacing: f32,
    opacity: f32,
) -> Option<TextBitmap> {
    if content.trim().is_empty() || font_size <= 0.0 {
        return None;
    }
    let color = super::skia_backend::parse_color(color_str, opacity);
    let total_width = engine.measure_width(content, font_size, font_id, letter_spacing);
    if total_width <= 0.0 {
        return None;
    }
    let align_offset = match align {
        "center" => total_width / 2.0,
        "right" => total_width,
        _ => 0.0,
    };

    // Bitmap layout: left padding, baseline placed `base_y` down from the top,
    // generous head/tail room for ascenders and descenders.
    let left_pad = (font_size * 0.3).ceil();
    let base_y = (font_size * 1.15).ceil();
    let width = (total_width + left_pad * 2.0).ceil().max(1.0) as u32;
    let height = (font_size * 1.6).ceil().max(1.0) as u32;

    let mut pm = Pixmap::new(width, height)?;

    let mut x_cursor = left_pad;
    for c in content.chars() {
        // Cached: the same glyph at the same size is rasterised from its
        // outline once, not once per frame, and the coverage is shared across
        // every colour it is ever drawn in.
        let glyph = match engine.glyph(c, font_size, font_id) {
            Some(g) => g,
            None => continue,
        };
        let metrics = glyph.metrics;
        let alpha = &glyph.alpha;
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
            let glyph_top = base_y - metrics.height as f32 - metrics.ymin as f32;
            let t = tiny_skia::Transform::from_translate(x_cursor + metrics.xmin as f32, glyph_top);
            pm.draw_pixmap(
                0,
                0,
                mask.as_ref(),
                &tiny_skia::PixmapPaint::default(),
                t,
                None,
            );
        }
        x_cursor += metrics.advance_width + letter_spacing;
    }

    Some(TextBitmap {
        rgba: pixmap_to_straight_rgba(&pm),
        width,
        height,
        place_x: -align_offset - left_pad,
        place_y: -base_y,
    })
}

/// Convert a premultiplied tiny-skia `Pixmap` to straight-alpha RGBA8 bytes,
/// the format `peniko::Image` (and therefore Vello `draw_image`) expects.
pub(crate) fn pixmap_to_straight_rgba(pm: &Pixmap) -> Vec<u8> {
    let mut out = Vec::with_capacity((pm.width() * pm.height() * 4) as usize);
    for px in pm.pixels() {
        let c: ColorU8 = px.demultiply();
        out.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    out
}

/// One simulated particle dot at the current time.
pub(crate) struct ParticleDot {
    pub x: f32,
    pub y: f32,
    pub r: f32,
    pub alpha: f32,
}

/// Deterministic analytical particle simulation shared by both backends. The
/// per-particle math is identical to the original Skia emitter so the GPU and
/// CPU backends produce the same dots for the same time.
#[allow(clippy::too_many_arguments)]
pub(crate) fn simulate_particles(
    count: u32,
    ex: f32,
    ey: f32,
    lifetime: f32,
    speed: f32,
    spread: f32,
    size: f32,
    opacity: f32,
    time: f32,
) -> Vec<ParticleDot> {
    if lifetime <= 0.0 {
        return Vec::new();
    }
    // The count is known before the loop, and this runs per emitter per frame.
    let mut dots = Vec::with_capacity(count as usize);
    for i in 0..count {
        let seed = hash01(i);
        let seed2 = hash01(i.wrapping_mul(2_654_435_761) ^ 0x9E37_79B9);
        let age = (time + seed * lifetime).rem_euclid(lifetime);
        let t = age / lifetime;
        let ang = if spread >= 360.0 {
            seed2 * std::f32::consts::TAU
        } else {
            let half = spread.to_radians() / 2.0;
            -std::f32::consts::FRAC_PI_2 + (seed2 * 2.0 - 1.0) * half
        };
        let v = speed * (0.6 + 0.4 * seed);
        let px = ex + ang.cos() * v * age;
        let py = ey + ang.sin() * v * age;
        let a = (1.0 - t) * opacity;
        if a <= 0.0 {
            continue;
        }
        let r = (size * (1.0 - 0.5 * t)).max(0.5);
        dots.push(ParticleDot {
            x: px,
            y: py,
            r,
            alpha: a.clamp(0.0, 1.0),
        });
    }
    dots
}

/// Deterministic hash → [0,1) used to seed particles reproducibly.
pub(crate) fn hash01(n: u32) -> f32 {
    let mut x = n.wrapping_add(0x9E37_79B9);
    x ^= x >> 16;
    x = x.wrapping_mul(0x21f0_aaad);
    x ^= x >> 15;
    x = x.wrapping_mul(0x735a_2d97);
    x ^= x >> 15;
    // Divide by 2^32, not by u32::MAX. f32 has a 24-bit mantissa, so
    // `u32::MAX as f32` rounds *up* to 4294967296.0 and large inputs then
    // round to exactly 1.0 — breaking the half-open range this function
    // documents and every caller assumes. Taking the top 24 bits keeps the
    // result exactly representable and strictly below 1.
    (x >> 8) as f32 / 16_777_216.0
}
