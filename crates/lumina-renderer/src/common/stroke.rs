//! Shared stroke helpers.

/// Dash pattern implementing progressive stroke reveal (`draw_fraction`):
/// one visible dash covering the leading `frac` of the total `length`,
/// followed by a gap longer than the geometry so nothing else draws.
pub(crate) fn draw_fraction_dash(frac: f32, length: f32) -> Vec<f32> {
    let frac = frac.clamp(0.0, 1.0);
    vec![length * frac, length * 2.0]
}
