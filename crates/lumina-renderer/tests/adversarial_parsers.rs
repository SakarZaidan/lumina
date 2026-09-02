//! Adversarial inputs for the parsers that read untrusted data.
//!
//! These are the same entry points the `fuzz/` targets drive, exercised over a
//! fixed corpus so they run on **stable, in CI, on every commit**. That
//! distinction matters: `cargo-fuzz` needs a nightly toolchain and sanitizer
//! flags, so a fuzz target on its own runs only when somebody remembers to run
//! it — which, on a repository that went quiet for seven weeks, is never.
//!
//! Fuzzing explores; this pins down what exploration has already found and the
//! shapes that break index-walking parsers in general. The two are
//! complementary, and neither replaces the other.
//!
//! The bar for every case here is the same: **return something, or return an
//! error — but do not panic, hang, or allocate without bound.**

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use luminafx_renderer::testing::{latex, path, plot};

/// Inputs that break parsers which walk a buffer by index.
fn hostile_strings() -> Vec<String> {
    let mut cases: Vec<String> = [
        "",
        " ",
        "\0",
        "\u{feff}",
        "\\",
        "{",
        "}",
        "{{{{",
        "}}}}",
        "^",
        "_",
        "^{",
        "_{",
        "\\frac",
        "\\frac{",
        "\\frac{a}",
        "\\frac{}{}",
        "\\sqrt{",
        "M",
        "M ",
        "M0",
        "M0,",
        "M 0 0 L",
        "M 0 0 C 1 1 2",
        "C 1 1 2 2 3 3",
        "Z",
        "zzzz",
        "M 0 0 L NaN NaN",
        "M 0 0 L inf inf",
        "M 1e400 0",
        "M -0 -0",
        "(",
        "((((((((",
        "1 +",
        "x /0",
        "math::",
        "math::sin",
        "sin",
        "sin(",
        "asin(",
        "sinsinsin(x)",
        "x ^ x ^ x ^ x",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();

    // Deep nesting and long runs: the classic ways to find a stack overflow or
    // an accidental quadratic.
    cases.push("(".repeat(200));
    cases.push("{".repeat(200));
    cases.push("\\frac{".repeat(100));
    cases.push("^".repeat(500));
    cases.push(format!("M {}", "0 ".repeat(2000)));
    cases.push("L".repeat(1000));
    cases.push(format!("math::sin({})", "x".repeat(1000)));
    cases.push("\u{10FFFF}".repeat(100));
    cases
}

#[test]
fn svg_path_parsing_survives_hostile_input() {
    for case in hostile_strings() {
        // Returning None is correct. Panicking is not.
        let _ = path::parse_svg_path(&case);
    }
}

#[test]
fn latex_transliteration_survives_hostile_input() {
    for case in hostile_strings() {
        let out = latex::latex_to_unicode(&case);
        // A transliterator must not turn a bounded input into an unbounded
        // one; a quadratic blow-up here is reachable straight from a scene.
        assert!(
            out.len() <= case.len().saturating_mul(64) + 1024,
            "input of {} bytes produced {} bytes: {case:?}",
            case.len(),
            out.len()
        );
    }
}

#[test]
fn plot_expression_handling_survives_hostile_input() {
    for case in hostile_strings() {
        let normalized = plot::normalize_math_calls(&case);
        // Rewriting must stay proportional to its input.
        assert!(
            normalized.len() <= case.len().saturating_mul(8) + 64,
            "normalising {} bytes produced {} bytes: {case:?}",
            case.len(),
            normalized.len()
        );
        // An unparseable expression yields no segments, and never panics.
        let _ = plot::sample(&case, -10.0, 10.0, -10.0, 10.0, 64);
    }
}

#[test]
fn plot_sampling_survives_hostile_ranges() {
    // Ranges come from an Axes object, which is scene data.
    let ranges = [
        (0.0, 0.0),
        (1.0, -1.0),
        (f64::NAN, 1.0),
        (0.0, f64::NAN),
        (f64::NEG_INFINITY, f64::INFINITY),
        (-1e300, 1e300),
        (f64::MIN, f64::MAX),
        (0.0, f64::MIN_POSITIVE),
    ];
    for (x_min, x_max) in ranges {
        for (y_min, y_max) in ranges {
            let segments = plot::sample("math::sin(x)", x_min, x_max, y_min, y_max, 64);
            let total: usize = segments.iter().map(Vec::len).sum();
            assert!(
                total <= 256,
                "range x[{x_min}, {x_max}] y[{y_min}, {y_max}] produced {total} points"
            );
        }
    }
}

#[test]
fn plot_sampling_respects_a_zero_budget() {
    // A zero or tiny budget must terminate immediately rather than falling
    // through to a minimum that ignores it.
    for budget in [0usize, 1, 2] {
        let _ = plot::sample("math::sin(x)", -10.0, 10.0, -10.0, 10.0, budget);
    }
}
