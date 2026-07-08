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

use fontdue::{Font, FontSettings};
use lumina_schema::TextProps;
use std::collections::HashMap;

#[derive(Default)]
pub struct TextEngine {
    /// Fonts stored in load order; first entry is the primary, rest are fallbacks.
    fonts: HashMap<String, Font>,
    /// Insertion order so fallback traversal is deterministic.
    font_order: Vec<String>,
}

impl TextEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load_font(&mut self, id: String, data: &[u8]) -> Result<(), String> {
        let font = Font::from_bytes(data, FontSettings::default())?;
        if !self.font_order.contains(&id) {
            self.font_order.push(id.clone());
        }
        self.fonts.insert(id, font);
        Ok(())
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
            if let Some(font) = self.font_for_char(c, font_id) {
                width += font.metrics(c, font_size).advance_width;
            }
        }
        if count > 1 {
            width += letter_spacing * (count as f32 - 1.0);
        }
        width
    }
}

pub struct GlyphInstance {
    pub glyph_char: char,
    pub x: f32,
    pub y: f32,
    pub font_size: f32,
    pub color: String,
}
