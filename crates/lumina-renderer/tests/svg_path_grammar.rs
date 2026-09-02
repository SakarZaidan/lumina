//! The SVG path grammar, and the parts of it that were silently missing.
//!
//! The previous parser handled `M L H V C Z` and ignored everything else, so
//! quadratics, smooth curves and elliptical arcs — which every vector editor
//! emits — were dropped without a word. It also did not implement repeated
//! coordinate sets, so `L 1 1 2 2` drew one line instead of two.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use lumina_renderer::testing::path::{parse_svg_path, PathData};

/// Parse, or fail with the parser's own diagnostic.
fn parse(d: &str) -> PathData {
    parse_svg_path(d).unwrap_or_else(|| panic!("should parse: {d:?}"))
}

/// Number of drawing commands, excluding the opening `MoveTo`.
fn draws(d: &str) -> usize {
    let text = format!("{:?}", parse(d));
    text.matches("To(").count() - text.matches("MoveTo(").count()
}

#[test]
fn the_original_command_set_still_works() {
    assert!(parse_svg_path("M 0 0 L 10 10 Z").is_some());
    assert!(parse_svg_path("M0 0H10V10Z").is_some());
    assert!(parse_svg_path("M 0 0 C 1 1 2 2 3 3").is_some());
    assert!(parse_svg_path("m 0 0 l 10 10 z").is_some());
}

#[test]
fn quadratic_curves_are_understood() {
    // `Q` was ignored entirely, so a shape built from quadratics drew nothing.
    let p = parse("M 0 0 Q 5 10 10 0");
    assert!(
        format!("{p:?}").contains("QuadTo"),
        "expected a QuadTo: {p:?}"
    );
}

#[test]
fn smooth_curves_reflect_the_previous_control_point() {
    // `S` and `T` mean "mirror the last control point", which is why they can
    // be shorter than the curve they continue.
    let s = parse("M 0 0 C 0 5 5 5 5 0 S 10 -5 10 0");
    assert_eq!(
        format!("{s:?}").matches("CubicTo").count(),
        2,
        "S must produce a second cubic: {s:?}"
    );
    let t = parse("M 0 0 Q 5 5 10 0 T 20 0");
    assert_eq!(
        format!("{t:?}").matches("QuadTo").count(),
        2,
        "T must produce a second quadratic: {t:?}"
    );
}

#[test]
fn elliptical_arcs_become_curves() {
    // Arcs are what rounded shapes from a vector editor are made of.
    let p = parse("M 0 0 A 10 10 0 0 1 20 0");
    assert!(
        format!("{p:?}").contains("CubicTo"),
        "an arc must produce curve segments: {p:?}"
    );
}

#[test]
fn a_full_circle_arc_closes_on_itself() {
    // Two semicircular arcs should return to the start. This is the check that
    // catches a sign error in the endpoint-to-centre conversion, which
    // otherwise produces a plausible-looking but wrong curve.
    let p = parse("M 0 0 A 10 10 0 1 1 20 0 A 10 10 0 1 1 0 0");
    let text = format!("{p:?}");
    let last = text.rsplit("CubicTo(").next().expect("at least one cubic");
    let nums: Vec<f32> = last
        .trim_end_matches(&['}', ']', ')'][..])
        .split(',')
        .filter_map(|t| t.trim().trim_end_matches(')').parse::<f32>().ok())
        .collect();
    let (ex, ey) = (nums[nums.len() - 2], nums[nums.len() - 1]);
    assert!(
        ex.abs() < 0.5 && ey.abs() < 0.5,
        "a full circle should end where it started, ended at ({ex}, {ey})"
    );
}

#[test]
fn arc_flags_change_which_arc_is_drawn() {
    // large-arc and sweep pick between four arcs through the same endpoints.
    // If the flags were ignored, all four would be identical.
    let variants: Vec<String> = [
        "M 0 0 A 10 10 0 0 0 10 10",
        "M 0 0 A 10 10 0 0 1 10 10",
        "M 0 0 A 10 10 0 1 0 10 10",
        "M 0 0 A 10 10 0 1 1 10 10",
    ]
    .iter()
    .map(|d| format!("{:?}", parse(d)))
    .collect();
    let distinct: std::collections::HashSet<&String> = variants.iter().collect();
    assert_eq!(
        distinct.len(),
        4,
        "the four flag combinations must draw four different arcs"
    );
}

#[test]
fn repeated_coordinate_sets_repeat_the_command() {
    // `L 1 1 2 2` is two lines. The old parser drew one and discarded the
    // rest, which is why complex paths came out truncated.
    assert_eq!(
        draws("M 0 0 L 1 1 2 2 3 3"),
        3,
        "three coordinate pairs, three lines"
    );
    assert_eq!(draws("M 0 0 C 1 1 2 2 3 3 4 4 5 5 6 6"), 2, "two cubics");
}

#[test]
fn a_repeat_after_moveto_becomes_a_lineto() {
    // The specification is explicit about this, and files rely on it.
    let p = parse("M 0 0 10 10 20 20");
    let text = format!("{p:?}");
    assert_eq!(
        text.matches("MoveTo").count(),
        1,
        "only the first is a move"
    );
    assert_eq!(
        text.matches("LineTo").count(),
        2,
        "the rest are lines: {text}"
    );
}

#[test]
fn numbers_may_run_together_without_separators() {
    // `M0 0-1-1` and `1.5.5` are both legal SVG, and both broke a
    // whitespace-splitting lexer.
    assert_eq!(draws("M0,0L10,10"), 1);
    assert_eq!(draws("M0 0L-1-1"), 1, "-1-1 is two numbers, not one");
    assert_eq!(draws("M0 0L1.5.5"), 1, "1.5 and .5 are two numbers");
    assert!(
        parse_svg_path("M0 0L1e2 1e2").is_some(),
        "exponents are legal"
    );
}

#[test]
fn a_degenerate_arc_becomes_a_line() {
    // The specification requires a zero radius to draw a straight line rather
    // than an error — a conforming renderer does not reject the file.
    let p = parse("M 0 0 A 0 0 0 0 1 10 10");
    assert!(format!("{p:?}").contains("LineTo"));
}

#[test]
fn errors_name_the_offending_token() {
    use lumina_renderer::testing::path::parse_svg_path_detailed;

    // Previously any of these silently dropped the whole path.
    let err = parse_svg_path_detailed("M 0 0 L 10 nonsense").expect_err("should fail");
    assert!(
        err.offset > 0 && !err.token.is_empty(),
        "the error must locate the problem: {err}"
    );

    let err = parse_svg_path_detailed("10 10 L 5 5").expect_err("coordinates before any command");
    assert!(err.expected.contains("command"), "{err}");

    let err = parse_svg_path_detailed("M 0 0 C 1 1").expect_err("truncated cubic");
    assert!(err.expected.contains("numbers"), "{err}");
}

#[test]
fn parsing_is_deterministic() {
    let d = "M 0 0 Q 5 10 10 0 T 20 0 A 5 5 0 0 1 30 0 Z";
    assert_eq!(format!("{:?}", parse(d)), format!("{:?}", parse(d)));
}
