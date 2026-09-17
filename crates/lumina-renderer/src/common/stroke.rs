//! Shared stroke helpers.

/// Dash pattern implementing progressive stroke reveal (`draw_fraction`):
/// one visible dash covering the leading `frac` of the total `length`,
/// followed by a gap longer than the geometry so nothing else draws.
pub(crate) fn draw_fraction_dash(frac: f32, length: f32) -> Vec<f32> {
    let frac = frac.clamp(0.0, 1.0);
    vec![length * frac, length * 2.0]
}

/// A validated dash pattern, or `None` when there is none to draw.
///
/// tiny-skia and kurbo both require a pattern of at least two positive
/// entries, and both misbehave on zeros or negatives — so a malformed pattern
/// draws solid rather than producing something neither backend agrees on.
/// An odd-length pattern is doubled, which is what SVG and Canvas do: `[5]`
/// means five on, five off.
pub(crate) fn dash_pattern(dash: Option<&[f32]>) -> Option<Vec<f32>> {
    let raw = dash?;
    if raw.is_empty() || raw.iter().any(|v| !v.is_finite() || *v < 0.0) {
        return None;
    }
    // All-zero would be an infinite loop for the rasteriser rather than a
    // pattern.
    if raw.iter().all(|v| *v <= 0.0) {
        return None;
    }
    let mut pattern = raw.to_vec();
    if pattern.len() % 2 == 1 {
        pattern.extend_from_slice(raw);
    }
    Some(pattern)
}

#[cfg(test)]
mod typed_equivalence {
    use super::dash_pattern;
    use serde_json::{json, Value};

    /// The JSON reading `dash_pattern` replaced, kept verbatim as the
    /// reference until RFC-0002 Stage 2 has moved every draw branch.
    fn dash_pattern_json(state: &Value) -> Option<Vec<f32>> {
        let raw = state.get("dash")?.as_array()?;
        let mut pattern: Vec<f32> = raw
            .iter()
            .filter_map(Value::as_f64)
            .map(|v| v as f32)
            .collect();

        if pattern.is_empty() || pattern.iter().any(|v| !v.is_finite() || *v < 0.0) {
            return None;
        }
        if pattern.iter().all(|v| *v <= 0.0) {
            return None;
        }
        if pattern.len() % 2 == 1 {
            let doubled = pattern.clone();
            pattern.extend(doubled);
        }
        Some(pattern)
    }

    #[test]
    fn dash_pattern_matches_the_json_reading() {
        let cases: [Option<Vec<f32>>; 9] = [
            None,
            Some(vec![]),
            Some(vec![5.0]),
            Some(vec![5.0, 3.0]),
            Some(vec![1.0, 2.0, 3.0]),
            Some(vec![0.0, 0.0]),
            Some(vec![0.0, 5.0]),
            Some(vec![-1.0, 2.0]),
            Some(vec![1e30, 0.5]),
        ];
        for dash in &cases {
            let state = json!({ "dash": dash });
            assert_eq!(
                dash_pattern(dash.as_deref()),
                dash_pattern_json(&state),
                "{state}"
            );
        }
    }
}
