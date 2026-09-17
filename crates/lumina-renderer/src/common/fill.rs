//! Shared fill/stroke resolution: JSON paint values → [`FillSpec`], plus
//! the bbox-derived gradient geometry both backends must agree on.

use luminafx_schema::Paint;

/// A resolved paint source, backend-neutral (straight-alpha RGBA stops).
#[derive(Debug, PartialEq)]
pub(crate) enum FillSpec {
    Solid([u8; 4]),
    Linear {
        stops: Vec<(f32, [u8; 4])>,
        angle_deg: f32,
    },
    Radial {
        stops: Vec<(f32, [u8; 4])>,
        radius_frac: f32,
    },
}

impl FillSpec {
    pub(crate) fn solid(hex: &str, opacity: f32) -> Self {
        FillSpec::Solid(super::color::parse_rgba8(hex, opacity))
    }
}

/// Resolve a paint into a [`FillSpec`].
///
/// A gradient of unknown `type` is linear, and a radial one without a `radius`
/// uses half the shape's extent. Fewer than two stops is `None`, so each
/// caller keeps its own fallback.
pub(crate) fn fill_spec(paint: &Paint, opacity: f32) -> Option<FillSpec> {
    match paint {
        Paint::Solid(hex) => Some(FillSpec::solid(hex, opacity)),
        Paint::Gradient(gradient) => {
            let stops = stop_specs(
                gradient.stops.iter().map(|(pos, hex)| (*pos, hex.as_str())),
                opacity,
            )?;
            match gradient.kind.as_str() {
                "radial" => Some(FillSpec::Radial {
                    stops,
                    radius_frac: gradient.radius.unwrap_or(0.5),
                }),
                _ => Some(FillSpec::Linear {
                    stops,
                    angle_deg: gradient.angle,
                }),
            }
        }
    }
}

/// Clamp, colour, sort and refine gradient stops; `None` below two.
fn stop_specs<'a>(
    stops: impl ExactSizeIterator<Item = (f32, &'a str)>,
    opacity: f32,
) -> Option<Vec<(f32, [u8; 4])>> {
    let mut out = Vec::with_capacity(stops.len());
    for (pos, hex) in stops {
        out.push((pos.clamp(0.0, 1.0), super::color::parse_rgba8(hex, opacity)));
    }
    if out.len() < 2 {
        return None;
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    Some(refine_perceptually(&out))
}

/// How many segments each adjacent stop pair is split into.
///
/// Both rasterisers interpolate **linearly in sRGB** between adjacent stops,
/// which is not the space the timeline blends colours in — so the same two
/// colours produced one midpoint in a gradient and a different one in a fade.
/// Neither backend exposes a choice of interpolation space, but both accept as
/// many stops as we care to give them, so sampling the perceptual curve and
/// handing over the samples gets the right result through an API that cannot
/// ask for it directly.
///
/// Eight segments keeps the maximum error from the piecewise-linear
/// approximation below one 8-bit step for the transitions where the two spaces
/// diverge most (saturated complementary hues); more stops would cost memory
/// for a difference nothing can display.
const GRADIENT_SEGMENTS: usize = 8;

/// Insert intermediate stops sampled from the perceptual blend.
///
/// Positions and colours of the author's own stops are preserved exactly; only
/// the path *between* them changes.
fn refine_perceptually(stops: &[(f32, [u8; 4])]) -> Vec<(f32, [u8; 4])> {
    let mut out = Vec::with_capacity(stops.len() + (stops.len() - 1) * (GRADIENT_SEGMENTS - 1));
    for pair in stops.windows(2) {
        let (p0, c0) = pair[0];
        let (p1, c1) = pair[1];
        out.push((p0, c0));
        // A zero-width span has nothing between it to refine, and would
        // otherwise emit a run of stops at one position.
        if p1 - p0 <= f32::EPSILON {
            continue;
        }
        for k in 1..GRADIENT_SEGMENTS {
            let t = k as f32 / GRADIENT_SEGMENTS as f32;
            out.push((
                p0 + (p1 - p0) * t,
                luminafx_core::interpolator::mix_rgba8(c0, c1, t),
            ));
        }
    }
    if let Some(last) = stops.last() {
        out.push(*last);
    }
    out
}

/// Linear-gradient axis derived from the shape bbox `(x, y, w, h)`: the
/// line through the bbox center along `angle_deg`, extended by half the
/// larger bbox side in each direction.
pub(crate) fn linear_geometry(
    bbox: (f32, f32, f32, f32),
    angle_deg: f32,
) -> ((f32, f32), (f32, f32)) {
    let (x, y, w, h) = bbox;
    let cx = x + w / 2.0;
    let cy = y + h / 2.0;
    let r = w.max(h) / 2.0;
    let rad = angle_deg.to_radians();
    let (dx, dy) = (rad.cos(), rad.sin());
    ((cx - dx * r, cy - dy * r), (cx + dx * r, cy + dy * r))
}

/// Radial-gradient center and radius derived from the shape bbox:
/// centered, radius = half the larger bbox side × `radius_frac`.
pub(crate) fn radial_geometry(bbox: (f32, f32, f32, f32), radius_frac: f32) -> ((f32, f32), f32) {
    let (x, y, w, h) = bbox;
    let cx = x + w / 2.0;
    let cy = y + h / 2.0;
    let r = (w.max(h) / 2.0) * radius_frac.max(0.01);
    ((cx, cy), r.max(0.01))
}

/// The typed helpers must answer exactly as the JSON ones do for the same
/// value, or moving a draw branch onto them would change pixels.
#[cfg(test)]
mod typed_equivalence {
    use super::{fill_spec, refine_perceptually, FillSpec};
    use luminafx_schema::{GradientSpec, Paint};
    use serde_json::Value;

    /// The JSON reading `fill_spec` replaced, kept verbatim as the reference
    /// until RFC-0002 Stage 2 has moved every draw branch.
    fn parse_fill(v: &Value, opacity: f32) -> Option<FillSpec> {
        match v {
            Value::String(s) => Some(FillSpec::solid(s, opacity)),
            Value::Object(map) => {
                let kind = map.get("type").and_then(|t| t.as_str()).unwrap_or("linear");
                let stops = parse_stops(map.get("stops"), opacity)?;
                match kind {
                    "radial" => {
                        let radius_frac =
                            map.get("radius").and_then(|r| r.as_f64()).unwrap_or(0.5) as f32;
                        Some(FillSpec::Radial { stops, radius_frac })
                    }
                    _ => {
                        let angle_deg =
                            map.get("angle").and_then(|a| a.as_f64()).unwrap_or(0.0) as f32;
                        Some(FillSpec::Linear { stops, angle_deg })
                    }
                }
            }
            _ => None,
        }
    }

    fn parse_stops(v: Option<&Value>, opacity: f32) -> Option<Vec<(f32, [u8; 4])>> {
        let arr = v?.as_array()?;
        let mut out = Vec::with_capacity(arr.len());
        for s in arr {
            let pair = s.as_array()?;
            let pos = pair.first()?.as_f64()? as f32;
            let hex = pair.get(1)?.as_str()?;
            out.push((
                pos.clamp(0.0, 1.0),
                crate::common::color::parse_rgba8(hex, opacity),
            ));
        }
        if out.len() < 2 {
            return None;
        }
        out.sort_by(|a, b| a.0.total_cmp(&b.0));
        Some(refine_perceptually(&out))
    }

    fn gradient(kind: &str, stops: &[(f32, &str)], angle: f32, radius: Option<f32>) -> Paint {
        Paint::Gradient(GradientSpec {
            kind: kind.to_string(),
            stops: stops.iter().map(|(p, c)| (*p, (*c).to_string())).collect(),
            angle,
            radius,
        })
    }

    #[test]
    fn fill_spec_matches_parse_fill() {
        let paints = [
            Paint::Solid("#FF0000".into()),
            Paint::Solid("#abc".into()),
            Paint::Solid("#FF000080".into()),
            Paint::Solid("not a colour".into()),
            gradient("linear", &[(0.0, "#FF0000"), (1.0, "#0000FF")], 45.0, None),
            gradient(
                "radial",
                &[(0.0, "#FFFFFF"), (1.0, "#000000")],
                0.0,
                Some(0.3),
            ),
            gradient("radial", &[(0.0, "#FFFFFF"), (1.0, "#000000")], 0.0, None),
            gradient("conic", &[(0.0, "#FFFFFF"), (1.0, "#000000")], 90.0, None),
            gradient("linear", &[(0.0, "#FF0000")], 0.0, None),
            gradient("linear", &[], 0.0, None),
            gradient(
                "linear",
                &[(1.5, "#00FF00"), (-0.5, "#FF0000"), (0.5, "#0000FF")],
                10.0,
                None,
            ),
        ];
        for paint in &paints {
            let json = serde_json::to_value(paint).expect("paint serialises");
            for opacity in [1.0, 0.5, 0.0] {
                assert_eq!(
                    fill_spec(paint, opacity),
                    parse_fill(&json, opacity),
                    "{json} at opacity {opacity}"
                );
            }
        }
    }
}
