//! Text engine for the Lumina animation engine.
//!
//! A thin layer over [`fontdue`]: font loading with deterministic,
//! insertion-ordered fallback ([`TextEngine::font_for_char`] tries the
//! preferred font first, then walks the remaining loaded fonts), and text
//! measurement with letter-spacing support.
//!
//! Glyph rasterization itself happens in `lumina-renderer` (shared between
//! the CPU and GPU backends); this crate only answers "which font can draw
//! this character" and "how wide is this string".

// The engine has never contained `unsafe`, and the metric tracking that was a
// `grep` over the source — which by v0.4.0 was returning a false positive from
// the word appearing in a comment. `forbid` makes it a compile error instead:
// it cannot be silenced by an `allow` further down, so a future `unsafe` block
// has to be argued for by removing this line, in a diff a reviewer will see.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

use fontdue::{Font, FontSettings, Metrics};
use luminafx_schema::TextProps;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

/// A glyph rasterised from its outline: fontdue's metrics plus the coverage
/// bitmap, one byte of alpha per pixel.
///
/// Deliberately colourless. Colour is applied when the glyph is composited, so
/// one cache entry serves every colour the same character is ever drawn in —
/// which is what makes caching worthwhile on a scene that fades text.
pub struct RasterizedGlyph {
    /// Advance, bearings, and bitmap dimensions, from fontdue.
    pub metrics: Metrics,
    /// Coverage, `metrics.width * metrics.height` bytes.
    pub alpha: Vec<u8>,
}

/// What identifies a rasterised glyph: which font, which character, what size.
///
/// The font is an index into `font_order` rather than a `String`, so a lookup
/// costs no allocation. The size is the `f32`'s exact bit pattern rather than
/// a rounded value — two sizes that differ below rounding really do produce
/// different bitmaps, and silently sharing one would be a rendering bug rather
/// than an optimisation.
type GlyphKey = (usize, char, u32);

/// Ceiling on cached glyphs before the cache is dropped and rebuilt.
///
/// An animated `font_size` produces a new key every frame, so an unbounded
/// cache would grow without limit across a long render. Clearing wholesale
/// rather than evicting least-recently-used keeps this predictable and cheap:
/// the pathological case rebuilds from scratch occasionally, and the common
/// case — a handful of sizes — never reaches the limit at all.
const MAX_CACHED_GLYPHS: usize = 8_192;

/// Font store with deterministic fallback: fonts are tried in load order,
/// so the same scene always resolves glyphs to the same font.
#[derive(Default)]
pub struct TextEngine {
    /// Fonts stored in load order; first entry is the primary, rest are fallbacks.
    fonts: HashMap<String, Font>,
    /// Insertion order so fallback traversal is deterministic.
    font_order: Vec<String>,
    /// Rasterised glyphs, keyed by font, character, and exact size.
    ///
    /// Glyphs were previously rasterised from outlines on every frame, and
    /// `font_for_char` walked every loaded font per character — twice, once to
    /// measure and once to draw. A fifty-character title over a minute of
    /// 60 fps output meant ~180 000 rasterisations of perhaps twenty distinct
    /// glyphs.
    ///
    /// `RefCell` because rasterisation happens behind `&self`, matching the
    /// SVG cache in the renderer. The engine is therefore not `Sync`, which is
    /// correct — a renderer is used by one thread at a time.
    ///
    /// `Arc` rather than `Rc` specifically so the engine stays **`Send`**. An
    /// `Rc` here compiles and passes every test, and then silently makes the
    /// whole renderer un-movable between threads, which is not discovered
    /// until somebody tries to build a pipelined export months later.
    /// `renderer_tests` asserts `Send` so that cannot happen quietly.
    glyphs: RefCell<HashMap<GlyphKey, Arc<RasterizedGlyph>>>,
}

impl TextEngine {
    /// An engine with no fonts loaded (text objects draw nothing until
    /// [`TextEngine::load_font`] is called).
    pub fn new() -> Self {
        Self::default()
    }

    /// Load a TTF/OTF font under `id`. Re-loading an existing id replaces
    /// its data but keeps its position in the fallback order.
    pub fn load_font(&mut self, id: String, data: &[u8]) -> Result<(), String> {
        let font = Font::from_bytes(data, FontSettings::default())?;
        if !self.font_order.contains(&id) {
            self.font_order.push(id.clone());
        }
        self.fonts.insert(id, font);
        // Re-loading an id keeps its index but replaces its outlines, so
        // anything cached against that index is now wrong.
        self.glyphs.borrow_mut().clear();
        Ok(())
    }

    /// Which font in load order will draw `c`, as an index into `font_order`.
    ///
    /// The index is what the glyph cache keys on, and it is stable for the
    /// lifetime of the engine because `load_font` never removes an entry from
    /// `font_order` — re-loading an id replaces the font data but keeps its
    /// position, so a cached glyph can never come back attributed to a
    /// different font.
    fn font_index_for_char(&self, c: char, preferred_id: Option<&str>) -> Option<usize> {
        if let Some(id) = preferred_id {
            if let Some(font) = self.fonts.get(id) {
                if font.metrics(c, 16.0).advance_width > 0.0 {
                    return self.font_order.iter().position(|f| f == id);
                }
            }
        }
        for (i, id) in self.font_order.iter().enumerate() {
            if Some(id.as_str()) == preferred_id {
                continue;
            }
            if let Some(font) = self.fonts.get(id) {
                if font.metrics(c, 16.0).advance_width > 0.0 {
                    return Some(i);
                }
            }
        }
        // Last resort: the first loaded font, so the glyph is at least
        // attempted — matching `font_for_char`.
        (!self.font_order.is_empty()).then_some(0)
    }

    /// Rasterise `c` at `font_size`, from cache when possible.
    ///
    /// Returns `None` only when no font is loaded at all. Colour is applied by
    /// the caller, so the returned coverage is shared across colours.
    pub fn glyph(
        &self,
        c: char,
        font_size: f32,
        preferred_id: Option<&str>,
    ) -> Option<Arc<RasterizedGlyph>> {
        let idx = self.font_index_for_char(c, preferred_id)?;
        let key: GlyphKey = (idx, c, font_size.to_bits());

        if let Some(hit) = self.glyphs.borrow().get(&key) {
            return Some(Arc::clone(hit));
        }

        let font = self.fonts.get(self.font_order.get(idx)?)?;
        let (metrics, alpha) = font.rasterize(c, font_size);
        let glyph = Arc::new(RasterizedGlyph { metrics, alpha });

        let mut cache = self.glyphs.borrow_mut();
        if cache.len() >= MAX_CACHED_GLYPHS {
            cache.clear();
        }
        cache.insert(key, Arc::clone(&glyph));
        Some(glyph)
    }

    /// Metrics for `c` at `font_size`, from the same cache as [`Self::glyph`].
    ///
    /// Measurement and drawing therefore agree by construction, and a string
    /// is rasterised once rather than once to measure and once to draw.
    pub fn glyph_metrics(
        &self,
        c: char,
        font_size: f32,
        preferred_id: Option<&str>,
    ) -> Option<Metrics> {
        self.glyph(c, font_size, preferred_id).map(|g| g.metrics)
    }

    /// Find the best font for a character: try the requested `font_id` first,
    /// then fall back through remaining loaded fonts in load order.
    pub fn font_for_char(&self, c: char, preferred_id: Option<&str>) -> Option<&Font> {
        // Try preferred font first.
        if let Some(id) = preferred_id {
            if let Some(font) = self.fonts.get(id) {
                if font.metrics(c, 16.0).advance_width > 0.0 {
                    return Some(font);
                }
            }
        }
        // Fallback: walk all fonts in load order.
        for id in &self.font_order {
            if Some(id.as_str()) == preferred_id {
                continue; // already tried
            }
            if let Some(font) = self.fonts.get(id) {
                if font.metrics(c, 16.0).advance_width > 0.0 {
                    return Some(font);
                }
            }
        }
        // Last resort: return any font so the glyph is at least attempted.
        self.font_order.first().and_then(|id| self.fonts.get(id))
    }

    /// Lay out a text object into per-glyph instances (position, size,
    /// color), advancing by each glyph's metric width.
    pub fn layout_text(&self, props: &TextProps) -> Vec<GlyphInstance> {
        let preferred = props.font_id.as_deref();
        let mut instances = Vec::new();
        let mut x_cursor = props.x;

        for c in props.content.chars() {
            if let Some(font) = self.font_for_char(c, preferred) {
                let metrics = font.metrics(c, props.font_size);
                instances.push(GlyphInstance {
                    glyph_char: c,
                    x: x_cursor,
                    y: props.y,
                    font_size: props.font_size,
                    color: props.color.clone(),
                });
                x_cursor += metrics.advance_width;
            }
        }

        instances
    }

    /// The font loaded under `id`, if any.
    pub fn get_font(&self, id: &str) -> Option<&Font> {
        self.fonts.get(id)
    }

    /// Return the preferred font for a given optional ID, with fallback to
    /// any loaded font. Used by the renderer for direct rasterization.
    pub fn resolve_font<'a>(&'a self, preferred_id: Option<&str>) -> Option<&'a Font> {
        if let Some(id) = preferred_id {
            if let Some(f) = self.fonts.get(id) {
                return Some(f);
            }
        }
        self.font_order.first().and_then(|id| self.fonts.get(id))
    }

    /// All loaded fonts, keyed by id.
    pub fn fonts(&self) -> &HashMap<String, Font> {
        &self.fonts
    }

    /// Total advance width of a string at a given size, including the extra
    /// `letter_spacing` between glyphs. Used by the renderer for alignment.
    pub fn measure_width(
        &self,
        content: &str,
        font_size: f32,
        font_id: Option<&str>,
        letter_spacing: f32,
    ) -> f32 {
        let mut width = 0.0;
        let count = content.chars().count();
        for c in content.chars() {
            if let Some(metrics) = self.glyph_metrics(c, font_size, font_id) {
                width += metrics.advance_width;
            }
        }
        if count > 1 {
            width += letter_spacing * (count as f32 - 1.0);
        }
        width
    }
}

/// One positioned glyph produced by [`TextEngine::layout_text`].
pub struct GlyphInstance {
    /// The character this glyph renders.
    pub glyph_char: char,
    /// Left edge of the glyph's advance box, in canvas coordinates.
    pub x: f32,
    /// Baseline y position, in canvas coordinates.
    pub y: f32,
    /// Font size in pixels.
    pub font_size: f32,
    /// Fill color (hex string, as authored in the scene).
    pub color: String,
}
