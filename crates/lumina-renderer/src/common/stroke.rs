//! Shared stroke helpers.

/// Dash pattern implementing progressive stroke reveal (`draw_fraction`):
/// one visible dash covering the leading `frac` of the total `length`,
/// followed by a gap longer than the geometry so nothing else draws.
pub(crate) fn draw_fraction_dash(frac: f32, length: f32) -> Vec<f32> {
    let frac = frac.clamp(0.0, 1.0);
    vec![length * frac, length * 2.0]
}

/// A validated dash pattern from scene state, or `None` when there is none.
///
/// tiny-skia and kurbo both require a pattern of at least two positive
/// entries, and both misbehave on zeros or negatives — so a malformed pattern
/// draws solid rather than producing something neither backend agrees on.
/// An odd-length pattern is doubled, which is what SVG and Canvas do: `[5]`
/// means five on, five off.
pub(crate) fn dash_pattern(state: &serde_json::Value) -> Option<Vec<f32>> {
    let raw = state.get("dash")?.as_array()?;
    let mut pattern: Vec<f32> = raw
        .iter()
        .filter_map(serde_json::Value::as_f64)
        .map(|v| v as f32)
        .collect();

    if pattern.is_empty() || pattern.iter().any(|v| !v.is_finite() || *v < 0.0) {
        return None;
    }
    // All-zero would be an infinite loop for the rasteriser rather than a
    // pattern.
    if pattern.iter().all(|v| *v <= 0.0) {
        return None;
    }
    if pattern.len() % 2 == 1 {
        let doubled = pattern.clone();
        pattern.extend(doubled);
    }
    Some(pattern)
}
