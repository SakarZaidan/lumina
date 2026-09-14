//! Text layout, shared by both backends.
//!
//! Where a glyph goes is a layout decision; how it gets onto the canvas is a
//! rendering one. The two were written twice — the CPU backend positioned
//! glyphs inline while the GPU backend positioned them into a standalone
//! bitmap — and the loops were the same arithmetic in two places, free to
//! drift apart (TD-18). They now share this module and differ only in the step
//! that is genuinely different: the CPU backend composites each glyph straight
//! into the frame, the GPU backend composites them into an image it then draws.

use std::sync::Arc;

use luminafx_text::{RasterizedGlyph, TextEngine};
use tiny_skia::{Color, Pixmap};

/// One glyph of a run, positioned relative to the run's anchor.
pub(crate) struct PlacedGlyph {
    /// The rasterised coverage, from the engine's cache.
    pub glyph: Arc<RasterizedGlyph>,
    /// Left edge of the glyph bitmap, in pixels right of the anchor `x`.
    /// Alignment is already folded in.
    pub dx: f32,
    /// Top edge of the glyph bitmap, in pixels below the baseline `y`.
    /// Always negative for anything above the baseline.
    pub dy: f32,
}

/// A laid-out text run: where every glyph goes.
pub(crate) struct TextLayout {
    /// Glyphs with a non-empty bitmap, in reading order. Spaces and control
    /// characters advance the cursor and are dropped here, since there is
    /// nothing to draw for them.
    ///
    /// Positions are relative to the run's anchor with alignment already
    /// folded in, so a caller needs nothing but the anchor to place them.
    pub glyphs: Vec<PlacedGlyph>,
}

/// Lay out `content`, resolving each character's font independently.
///
/// Returns `None` when there is nothing to draw — an empty string, a
/// non-positive size, or a run that measures zero wide.
pub(crate) fn layout_run(
    engine: &TextEngine,
    content: &str,
    font_size: f32,
    font_id: Option<&str>,
    align: &str,
    letter_spacing: f32,
) -> Option<TextLayout> {
    if content.is_empty() || font_size <= 0.0 {
        return None;
    }
    let width = engine.measure_width(content, font_size, font_id, letter_spacing);
    let align_offset = match align {
        "center" => width / 2.0,
        "right" => width,
        _ => 0.0,
    };

    let mut glyphs = Vec::new();
    let mut cursor = -align_offset;
    for c in content.chars() {
        // The engine's cache, so a glyph is rasterised from its outline once
        // rather than once per frame — and once for both backends rather than
        // once each.
        let Some(glyph) = engine.glyph(c, font_size, font_id) else {
            continue;
        };
        let metrics = glyph.metrics;
        if metrics.width > 0 && metrics.height > 0 {
            glyphs.push(PlacedGlyph {
                // fontdue's `ymin` is the signed distance from the baseline to
                // the glyph's bottom, so the top sits `height + ymin` above it.
                dy: -(metrics.height as f32) - metrics.ymin as f32,
                dx: cursor + metrics.xmin as f32,
                glyph,
            });
        }
        cursor += metrics.advance_width + letter_spacing;
    }

    Some(TextLayout { glyphs })
}

/// Where a glyph's bitmap goes: an integer pixel offset, with the fractional
/// remainder baked into the bitmap itself by [`glyph_mask`].
pub(crate) struct GlyphPlacement {
    /// Integer x offset from the run's anchor.
    pub ix: i32,
    /// Integer y offset from the run's baseline.
    pub iy: i32,
    /// Sub-pixel x remainder in `[0, 1)`, already applied to the mask.
    pub fx: f32,
    /// Sub-pixel y remainder in `[0, 1)`, already applied to the mask.
    pub fy: f32,
}

impl PlacedGlyph {
    /// Split this glyph's position into the integer part a compositor can
    /// translate by and the fraction that has to be drawn into the bitmap.
    ///
    /// `anchor_x`/`anchor_y` are the run's origin in the space the glyph will
    /// be composited in, so the split is taken where it matters rather than
    /// relative to the run.
    pub(crate) fn place(&self, anchor_x: f32, anchor_y: f32) -> GlyphPlacement {
        let px = anchor_x + self.dx;
        let py = anchor_y + self.dy;
        let ix = px.floor();
        let iy = py.floor();
        GlyphPlacement {
            ix: ix as i32,
            iy: iy as i32,
            fx: px - ix,
            fy: py - iy,
        }
    }
}

/// Tint a glyph's coverage with `color` and shift it by a sub-pixel offset.
///
/// Coverage is cached colourless — one entry serves every colour the character
/// is ever drawn in — so the tint happens here, at the point of use.
///
/// The sub-pixel shift is here rather than in the transform because neither
/// compositor will do it. `tiny_skia::Pixmap::draw_pixmap` snaps a translation
/// to whole pixels regardless of filter quality, so text drawn that way had
/// **one position per pixel**: a caption drifting across the screen jumped a
/// whole pixel at a time, and because each glyph crossed its own boundary at a
/// different moment, the spacing between letters visibly wobbled as it moved.
/// Baking the fraction into the coverage gives continuous motion, and gives
/// the two backends the same bitmap at the same integer offset instead of two
/// different roundings of the same position.
///
/// The mask is one pixel wider and taller than the glyph, since a shifted
/// glyph spills into the neighbouring row and column.
pub(crate) fn glyph_mask(
    glyph: &RasterizedGlyph,
    color: Color,
    fx: f32,
    fy: f32,
) -> Option<Pixmap> {
    let m = glyph.metrics;
    let (gw, gh) = (m.width, m.height);
    let (w, h) = (gw + 1, gh + 1);
    let mut mask = Pixmap::new(w as u32, h as u32)?;

    // Coverage at a fractional position, by bilinear sampling of the
    // rasterised bitmap. Outside it there is no ink.
    let coverage = |sx: i32, sy: i32| -> f32 {
        if sx < 0 || sy < 0 || sx >= gw as i32 || sy >= gh as i32 {
            0.0
        } else {
            f32::from(glyph.alpha[sy as usize * gw + sx as usize])
        }
    };

    let pixels = mask.pixels_mut();
    for y in 0..h {
        for x in 0..w {
            // Sampling at `-f` shifts the image by `+f`.
            let sx = x as i32 - 1;
            let sy = y as i32 - 1;
            let (wx, wy) = (fx, fy);
            let a = coverage(sx + 1, sy + 1) * (1.0 - wx) * (1.0 - wy)
                + coverage(sx, sy + 1) * wx * (1.0 - wy)
                + coverage(sx + 1, sy) * (1.0 - wx) * wy
                + coverage(sx, sy) * wx * wy;
            if a <= 0.0 {
                continue;
            }
            let final_a = a / 255.0 * color.alpha();
            pixels[y * w + x] = Color::from_rgba(color.red(), color.green(), color.blue(), final_a)
                .unwrap_or(Color::WHITE)
                .premultiply()
                .to_color_u8();
        }
    }
    Some(mask)
}
