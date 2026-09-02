//! Behaviour of the shared plot sampler.
//!
//! Sampling decides what a plotted function *looks like*, so these are about
//! visual correctness rather than plumbing: does a steep curve get the points
//! it needs, does a pole cut the line instead of drawing a false vertical, and
//! does `asin` mean arcsine.

// Integration tests are not `#[cfg(test)]` items, so clippy.toml's
// allow-in-tests does not reach them.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use lumina_renderer::testing::plot;

#[test]
fn bare_math_calls_are_namespaced() {
    assert_eq!(plot::normalize_math_calls("sin(x)"), "math::sin(x)");
    assert_eq!(
        plot::normalize_math_calls("sin(x) + cos(x)"),
        "math::sin(x) + math::cos(x)"
    );
}

#[test]
fn inverse_trig_is_not_mangled() {
    // `str::replace("sin(", "math::sin(")` turned `asin(x)` into
    // `amath::sin(x)`, which is not a function and evaluated to nothing.
    assert_eq!(plot::normalize_math_calls("asin(x)"), "math::asin(x)");
    assert_eq!(plot::normalize_math_calls("acos(x)"), "math::acos(x)");
    assert_eq!(plot::normalize_math_calls("atan(x)"), "math::atan(x)");
}

#[test]
fn already_namespaced_calls_are_left_alone() {
    assert_eq!(plot::normalize_math_calls("math::sin(x)"), "math::sin(x)");
    // Mixing styles used to abandon normalisation for the whole expression,
    // so the bare half silently stopped working.
    assert_eq!(
        plot::normalize_math_calls("math::sin(x) + cos(x)"),
        "math::sin(x) + math::cos(x)"
    );
}

#[test]
fn variables_and_non_math_identifiers_are_untouched() {
    assert_eq!(plot::normalize_math_calls("x * 2"), "x * 2");
    assert_eq!(plot::normalize_math_calls("x + sinner"), "x + sinner");
}

#[test]
fn inverse_trig_evaluates_correctly() {
    // The real test of the above: asin(1) is pi/2, not an error.
    let segments = plot::sample("asin(x)", 0.0, 1.0, -2.0, 2.0, 200);
    let last = segments
        .last()
        .and_then(|s| s.last())
        .expect("asin must produce points");
    assert!(
        (last.1 - std::f64::consts::FRAC_PI_2).abs() < 1e-3,
        "asin(1) should be pi/2, sampler ended at {last:?}"
    );
}

#[test]
fn a_straight_line_does_not_spend_its_budget() {
    // Refinement should not trigger at all on something already flat.
    //
    // This deliberately does not assert a small absolute count. Sampling seeds
    // with a uniform pass before refining, because a feature that is never
    // sampled cannot be detected — with a tiny seed, `sin(50x)` over a wide
    // domain aliases into nonsense. So the property is "a flat curve costs a
    // fraction of its budget", not "a flat curve costs N points", which would
    // just be pinning the seed size in a test.
    let budget = 4000;
    let total: usize = plot::sample("x", -10.0, 10.0, -10.0, 10.0, budget)
        .iter()
        .map(Vec::len)
        .sum();
    assert!(
        total < budget / 10,
        "a straight line took {total} of {budget} points; refinement is firing on a flat curve"
    );
}

#[test]
fn a_wiggly_curve_gets_more_points_than_a_flat_one() {
    let flat: usize = plot::sample("0.001 * x", -10.0, 10.0, -10.0, 10.0, 4000)
        .iter()
        .map(Vec::len)
        .sum();
    let wiggly: usize = plot::sample("math::sin(20 * x)", -10.0, 10.0, -2.0, 2.0, 4000)
        .iter()
        .map(Vec::len)
        .sum();
    assert!(
        wiggly > flat * 4,
        "curvature should attract samples: flat {flat}, wiggly {wiggly}"
    );
}

#[test]
fn a_pole_splits_the_curve_instead_of_crossing_it() {
    // tan(x) has an asymptote at pi/2. A single polyline through it draws a
    // false near-vertical line across the whole plot.
    let segments = plot::sample("tan(x)", 0.0, 3.0, -5.0, 5.0, 2000);
    assert!(
        segments.len() >= 2,
        "tan over [0, 3] must break at pi/2, got {} segment(s)",
        segments.len()
    );
}

#[test]
fn a_curve_reaches_toward_its_asymptote() {
    // Without bisecting toward the break the curve stops wherever the grid
    // happened to land, visibly short of the pole.
    let segments = plot::sample("tan(x)", 0.0, 1.5, -20.0, 20.0, 2000);
    let last_x = segments
        .last()
        .and_then(|s| s.last())
        .map(|p| p.0)
        .expect("tan must produce points");
    assert!(
        last_x > 1.49,
        "curve stopped at x = {last_x}, well short of the domain end"
    );
}

#[test]
fn sampling_respects_its_budget() {
    // sample_count is author-supplied and re-sampled every frame, so the
    // budget has to be a real bound, not a suggestion.
    for budget in [1usize, 10, 50, 500] {
        let total: usize = plot::sample("math::sin(50 * x)", -10.0, 10.0, -2.0, 2.0, budget)
            .iter()
            .map(Vec::len)
            .sum();
        assert!(
            total <= budget * 2,
            "budget {budget} produced {total} points"
        );
    }
}

#[test]
fn sampling_is_deterministic() {
    let a = plot::sample("math::sin(x) * x", -5.0, 5.0, -5.0, 5.0, 500);
    let b = plot::sample("math::sin(x) * x", -5.0, 5.0, -5.0, 5.0, 500);
    assert_eq!(a, b, "the same plot must sample identically every time");
}

#[test]
fn a_malformed_expression_yields_nothing_rather_than_panicking() {
    assert!(plot::sample("this is not (an expression", 0.0, 1.0, 0.0, 1.0, 100).is_empty());
    assert!(plot::sample("", 0.0, 1.0, 0.0, 1.0, 100).is_empty());
}

#[test]
fn a_wide_domain_keeps_its_resolution() {
    // f32 gives ~7 significant digits, so an x of 1e6 could not resolve steps
    // of 1. Sampling in f64 keeps the domain meaningful.
    let segments = plot::sample("x - 1000000", 1_000_000.0, 1_000_010.0, -20.0, 20.0, 200);
    let points: Vec<_> = segments.iter().flatten().collect();
    assert!(points.len() >= 2);
    let xs: Vec<f64> = points.iter().map(|p| p.0).collect();
    assert!(
        xs.windows(2).all(|w| w[1] > w[0]),
        "x values collapsed to duplicates at large magnitude: {xs:?}"
    );
}
