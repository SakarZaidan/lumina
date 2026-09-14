//! `draw_fraction` must mean "this much of the ink is on the canvas".
//!
//! Three implementations disagreed about that. A `Line` dashed the stroke,
//! which is exact for a straight line and rasteriser-dependent for anything
//! else. A `BezierCurve` cut at curve *parameter*, which is exact arithmetic
//! measuring the wrong quantity — a cubic traversed at uniform `t` does not
//! move at uniform speed, so the reveal accelerated and slowed while the
//! fraction climbed steadily. And `Path` had the field in its schema and
//! ignored it entirely, so a Path with a reveal animation simply appeared
//! whole.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use luminafx_renderer::testing::path::{length, parse_svg_path, trim};

#[test]
fn a_half_trim_is_half_the_length() {
    // The property, stated directly. An L-shape so the answer differs from
    // the straight-line distance between the endpoints.
    let p = parse_svg_path("M 0 0 L 100 0 L 100 100").expect("parses");
    let full = length(&p);
    assert!((full - 200.0).abs() < 0.5, "total length was {full}");

    for frac in [0.25f32, 0.5, 0.75] {
        let cut = length(&trim(&p, frac));
        assert!(
            (cut - full * frac).abs() < full * 0.01,
            "at {frac}: trimmed length {cut}, expected {}",
            full * frac
        );
    }
}

#[test]
fn trimming_is_proportional_on_a_curve() {
    // The case parameter-space cutting got wrong. This curve's control points
    // are bunched at one end, so equal steps in `t` cover very unequal
    // distances; equal steps in arc length must not.
    let p = parse_svg_path("M 0 0 C 90 0 100 10 100 100").expect("parses");
    let full = length(&p);
    let mut previous = 0.0f32;
    for step in 1..=8 {
        let frac = step as f32 / 8.0;
        let cut = length(&trim(&p, frac));
        let delta = cut - previous;
        // Every eighth of the fraction must add about an eighth of the length.
        assert!(
            (delta - full / 8.0).abs() < full * 0.03,
            "step {step}: added {delta}, expected about {}",
            full / 8.0
        );
        previous = cut;
    }
}

#[test]
fn the_endpoints_are_exact() {
    let p = parse_svg_path("M 10 10 C 40 0 60 20 90 10").expect("parses");
    assert!(trim(&p, 0.0).to_string_len() == 0, "nothing is drawn at 0");
    let full = length(&p);
    let whole = length(&trim(&p, 1.0));
    assert!(
        (whole - full).abs() < 1e-3,
        "at 1.0 the whole path must be drawn: {whole} vs {full}"
    );
}

#[test]
fn trimming_spans_subpaths_continuously() {
    // Two separate strokes should reveal one after the other, as one drawing,
    // rather than each revealing at its own rate simultaneously.
    let p = parse_svg_path("M 0 0 L 100 0 M 0 50 L 100 50").expect("parses");
    let full = length(&p);
    assert!((full - 200.0).abs() < 0.5);

    // A quarter of the total is half of the first stroke, and none of the
    // second.
    let quarter = trim(&p, 0.25);
    assert!(
        (length(&quarter) - 50.0).abs() < 2.0,
        "quarter of two 100-unit strokes is 50 units, got {}",
        length(&quarter)
    );
}

#[test]
fn a_degenerate_path_does_not_panic() {
    for d in ["M 0 0", "M 0 0 L 0 0", "M 5 5 L 5 5 L 5 5"] {
        let p = parse_svg_path(d).expect("parses");
        for frac in [-1.0f32, 0.0, 0.5, 1.0, 2.0] {
            let _ = trim(&p, frac);
        }
    }
}

/// Helper: number of commands, since `PathData`'s contents are crate-private.
trait CmdCount {
    fn to_string_len(&self) -> usize;
}
impl CmdCount for luminafx_renderer::testing::path::PathData {
    fn to_string_len(&self) -> usize {
        format!("{self:?}").matches("To(").count()
    }
}
