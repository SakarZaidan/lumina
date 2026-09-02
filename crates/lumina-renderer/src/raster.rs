//! Shared CPU rasterization helpers used by both backends.
//!
//! The GPU (Vello) backend reuses the proven fontdue / tiny-skia raster path
//! for text, math and assets by producing straight-alpha RGBA buffers that are
//! wrapped in a `peniko::Image` and composited via `Scene::draw_image`. This
//! avoids maintaining a second (skrifa) font stack while reaching parity with
//! the Skia backend. Particle simulation is shared so both backends emit
//! byte-identical, scrub-reproducible output.

use luminafx_text::TextEngine;
use tiny_skia::{ColorU8, Pixmap};

/// One glyph of a run as a standalone straight-alpha bitmap, positioned
/// relative to the run's anchor.
pub(crate) struct GlyphBitmap {
    /// Straight-alpha RGBA8, `width * height * 4` bytes.
    pub rgba: Vec<u8>,
    /// Bitmap width in pixels.
    pub width: u32,
    /// Bitmap height in pixels.
    pub height: u32,
    /// Left edge, as a whole-pixel offset right of the anchor `x`; alignment
    /// folded in, and the sub-pixel remainder already baked into `rgba`.
    pub ix: i32,
    /// Top edge, as a whole-pixel offset below the baseline `y`.
    pub iy: i32,
}

/// Rasterise a text run as one bitmap **per glyph**.
///
/// The GPU backend used to composite the whole string into a single bitmap and
/// draw that as one image, which meant the glyphs were resampled twice: once
/// placing them into the string bitmap at fractional offsets, and again when
/// that bitmap was drawn under the scene transform. The CPU backend resampled
/// once. That second pass is where their text diverged (TD-18).
///
/// Drawing a glyph at a time gives each backend the same source bitmap at the
/// same position with one resample each, so only the rasteriser's own sampling
/// differs.
#[allow(clippy::too_many_arguments)]
pub(crate) fn rasterize_glyphs(
    engine: &TextEngine,
    content: &str,
    font_size: f32,
    color_str: &str,
    font_id: Option<&str>,
    align: &str,
    letter_spacing: f32,
    opacity: f32,
    anchor_x: f32,
    anchor_y: f32,
) -> Vec<GlyphBitmap> {
    if content.trim().is_empty() || font_size <= 0.0 {
        return Vec::new();
    }
    let color = super::skia_backend::parse_color(color_str, opacity);
    let Some(layout) =
        crate::common::text::layout_run(engine, content, font_size, font_id, align, letter_spacing)
    else {
        return Vec::new();
    };

    layout
        .glyphs
        .iter()
        .filter_map(|placed| {
            let at = placed.place(anchor_x, anchor_y);
            let mask = crate::common::text::glyph_mask(&placed.glyph, color, at.fx, at.fy)?;
            Some(GlyphBitmap {
                width: mask.width(),
                height: mask.height(),
                rgba: pixmap_to_straight_rgba(&mask),
                ix: at.ix,
                iy: at.iy,
            })
        })
        .collect()
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
