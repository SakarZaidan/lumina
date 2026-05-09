use lumina_schema::TextProps;
use fontdue::{Font, FontSettings};
use std::collections::HashMap;

pub struct TextEngine {
    fonts: HashMap<String, Font>,
}

impl TextEngine {
    pub fn new() -> Self {
        Self {
            fonts: HashMap::new(),
        }
    }

    pub fn load_font(&mut self, id: String, data: &[u8]) -> Result<(), String> {
        let font = Font::from_bytes(data, FontSettings::default())?;
        self.fonts.insert(id, font);
        Ok(())
    }

    pub fn layout_text(&self, props: &TextProps) -> Vec<GlyphInstance> {
        let font = if let Some(font_id) = &props.font_id {
            self.fonts.get(font_id)
        } else {
            self.fonts.values().next()
        };

        let font = match font {
            Some(f) => f,
            None => return vec![],
        };

        let mut instances = Vec::new();
        let mut x_cursor = props.x;
        
        for c in props.content.chars() {
            let metrics = font.metrics(c, props.font_size);
            instances.push(GlyphInstance {
                glyph_char: c,
                x: x_cursor,
                y: props.y, // Simplistic baseline
                font_size: props.font_size,
                color: props.color.clone(),
            });
            x_cursor += metrics.advance_width;
        }

        instances
    }

    pub fn get_font(&self, id: &str) -> Option<&Font> {
        self.fonts.get(id)
    }

    pub fn fonts(&self) -> &HashMap<String, Font> {
        &self.fonts
    }
}

pub struct GlyphInstance {
    pub glyph_char: char,
    pub x: f32,
    pub y: f32,
    pub font_size: f32,
    pub color: String,
}
