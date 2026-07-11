//! Shared color parsing.

/// Parse `#rgb` / `#rrggbb` / `#rrggbbaa` into straight-alpha RGBA bytes,
/// with `opacity` multiplied into the alpha channel. Unparseable input
/// falls back to opaque white (historical behavior of both backends).
pub(crate) fn parse_rgba8(hex: &str, opacity: f32) -> [u8; 4] {
    let hex = hex.trim_start_matches('#');
    match hex.len() {
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255);
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(255);
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(255);
            [r, g, b, (opacity * 255.0) as u8]
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255);
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(255);
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(255);
            let a = u8::from_str_radix(&hex[6..8], 16).unwrap_or(255);
            [r, g, b, (a as f32 * opacity) as u8]
        }
        3 => {
            // Short form: #RGB → #RRGGBB
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).unwrap_or(255);
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).unwrap_or(255);
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).unwrap_or(255);
            [r, g, b, (opacity * 255.0) as u8]
        }
        _ => [255, 255, 255, 255],
    }
}

/// Adapter: shared RGBA bytes → tiny-skia color.
pub(crate) fn to_tiny(c: [u8; 4]) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba8(c[0], c[1], c[2], c[3])
}

/// Adapter: shared RGBA bytes → peniko (vello) color.
pub(crate) fn to_peniko(c: [u8; 4]) -> vello::peniko::Color {
    vello::peniko::Color::rgba8(c[0], c[1], c[2], c[3])
}
