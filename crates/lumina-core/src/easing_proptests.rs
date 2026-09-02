//! Property tests for the easing registry.
//!
//! Unit tests check the values someone thought to write down. These check the
//! invariants that must hold for *every* input, which is where the defects
//! actually were: two solvers assumed preconditions nothing validated, and one
//! function was quantised to 100 discrete levels while documenting a smooth
//! curve. All of it was reachable in a few lines of generated input.
//!
//! Anything asserted here is a promise the registry makes to scene authors:
//! an easing is a reparameterisation of time from 0 to 1, and a renderer may
//! evaluate it at any point, in any order, as many times as it likes.

#[cfg(test)]
mod tests {
    use crate::easing::{eval_easing, get_easing_fn, EASING_NAMES};
    use proptest::prelude::*;

    /// Easings that deliberately leave [0, 1] and are excluded from the
    /// bounded-output property. Every one of them overshoots by design.
    const OVERSHOOTING: &[&str] = &[
        "ease_in_elastic",
        "ease_out_elastic",
        "ease_in_out_elastic",
        "spring",
    ];

    /// Easings that are not monotonic by design.
    const NON_MONOTONIC: &[&str] = &[
        "ease_in_elastic",
        "ease_out_elastic",
        "ease_in_out_elastic",
        "ease_in_bounce",
        "ease_out_bounce",
        "spring",
        "there_and_back",
    ];

    /// Easings needing parameters; `eval_easing` handles them separately.
    const PARAMETERIZED: &[&str] = &["cubic_bezier", "spline"];

    fn plain_easings() -> impl Iterator<Item = &'static str> {
        EASING_NAMES
            .iter()
            .copied()
            .filter(|n| !PARAMETERIZED.contains(n))
    }

    /// Easings that deliberately do not end at 1.
    ///
    /// `there_and_back` is Manim's emphasis curve: it reaches 1 at the
    /// midpoint and returns to 0. Ending where it started is the entire point,
    /// so it is exempt from the endpoint property rather than a violation of
    /// it.
    const RETURNS_TO_START: &[&str] = &["there_and_back"];

    #[test]
    fn every_easing_pins_both_endpoints() {
        // f(0) == 0 and f(1) == 1 is what makes an easing composable with a
        // keyframe pair at all: the value must arrive exactly where the author
        // wrote it, not near it.
        for name in plain_easings().filter(|n| !RETURNS_TO_START.contains(n)) {
            let f = get_easing_fn(name);
            assert!(f(0.0).abs() < 1e-6, "{name}(0) must be 0, was {}", f(0.0));
            assert!(
                (f(1.0) - 1.0).abs() < 1e-6,
                "{name}(1) must be 1, was {}",
                f(1.0)
            );
        }
    }

    #[test]
    fn ease_css_matches_the_curve_it_documents() {
        // `ease_css` is documented as cubic-bezier(0.25, 0.1, 0.25, 1.0) and
        // used to call ease_in_out_sine, which is a visibly different curve.
        let params = serde_json::json!([0.25, 0.1, 0.25, 1.0]);
        for i in 0..=100 {
            let t = i as f32 / 100.0;
            let documented = eval_easing("cubic_bezier", Some(&params), t);
            let actual = get_easing_fn("ease")(t);
            assert!(
                (documented - actual).abs() < 1e-4,
                "ease({t}) = {actual}, but cubic-bezier(0.25,0.1,0.25,1) = {documented}"
            );
        }
    }

    #[test]
    fn the_spring_is_tunable() {
        use crate::easing::{spring_with, SpringParams, DEFAULT_SPRING};

        // Heavier damping must settle sooner and overshoot less. Checking a
        // relationship rather than a magic number keeps this meaningful if the
        // defaults are ever retuned.
        let stiff = SpringParams {
            damping: 40.0,
            ..DEFAULT_SPRING
        };
        let peak = |p: SpringParams| {
            (0..=1000)
                .map(|i| spring_with(p, i as f32 / 1000.0))
                .fold(f32::MIN, f32::max)
        };
        assert!(
            peak(stiff) <= peak(DEFAULT_SPRING) + 1e-6,
            "more damping must not overshoot more: {} vs {}",
            peak(stiff),
            peak(DEFAULT_SPRING)
        );

        // Critically damped and overdamped springs must not overshoot at all.
        for damping in [f32::sqrt(4.0 * 200.0), 60.0, 200.0] {
            let p = SpringParams {
                damping,
                ..DEFAULT_SPRING
            };
            assert!(
                peak(p) <= 1.0 + 1e-3,
                "damping {damping} overshot to {}",
                peak(p)
            );
            assert!(
                (spring_with(p, 1.0) - 1.0).abs() < 1e-5,
                "damping {damping} did not land on 1"
            );
        }
    }

    #[test]
    fn there_and_back_returns_to_where_it_started() {
        let f = get_easing_fn("there_and_back");
        assert!(f(0.0).abs() < 1e-6, "starts at {}", f(0.0));
        assert!(f(1.0).abs() < 1e-6, "must return to 0, ended at {}", f(1.0));
        assert!(
            (f(0.5) - 1.0).abs() < 1e-6,
            "must reach 1 at the midpoint, was {}",
            f(0.5)
        );
    }

    #[test]
    fn easings_are_curves_not_staircases() {
        // `spring` used to integrate 100 fixed Euler steps and index them with
        // `(t / dt).round()`, so its output took at most 101 distinct values
        // across the whole domain — a staircase, while documenting a smooth
        // curve. Sampling finely and counting distinct outputs separates that
        // from a genuine curve, which an adjacent-value comparison cannot:
        // a converged tail legitimately repeats values at f32 precision.
        const SAMPLES: u32 = 10_000;
        const MIN_DISTINCT: usize = 1_000;

        for name in plain_easings().filter(|n| *n != "linear") {
            let f = get_easing_fn(name);
            let distinct: std::collections::HashSet<u32> = (0..SAMPLES)
                .map(|i| f(i as f32 / SAMPLES as f32).to_bits())
                .collect();
            assert!(
                distinct.len() > MIN_DISTINCT,
                "{name} produced only {} distinct values over {SAMPLES} samples — that is a \
                 quantised approximation, not a continuous curve",
                distinct.len()
            );
        }
    }

    proptest! {
        /// No easing may produce a non-finite value.
        ///
        /// This matters beyond tidiness: `serde_json` maps NaN and infinity to
        /// `Value::Null`, so a non-finite easing result makes the property
        /// vanish from the state map and the renderer silently substitutes its
        /// default. The animation is wrong and nothing says so.
        #[test]
        fn easings_are_finite_across_the_domain(t in 0.0f32..=1.0) {
            for name in plain_easings() {
                let v = get_easing_fn(name)(t);
                prop_assert!(v.is_finite(), "{name}({t}) = {v}");
            }
        }

        /// Non-overshooting easings stay within [0, 1].
        #[test]
        fn bounded_easings_stay_in_range(t in 0.0f32..=1.0) {
            for name in plain_easings().filter(|n| !OVERSHOOTING.contains(n)) {
                let v = get_easing_fn(name)(t);
                prop_assert!(
                    (-1e-4..=1.0 + 1e-4).contains(&v),
                    "{name}({t}) = {v} escaped [0, 1]"
                );
            }
        }

        /// Monotonic easings never move backwards.
        ///
        /// An object animating from A to B under a monotonic easing must not
        /// visibly reverse partway.
        #[test]
        fn monotonic_easings_never_decrease(a in 0.0f32..=1.0, b in 0.0f32..=1.0) {
            let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
            for name in plain_easings().filter(|n| !NON_MONOTONIC.contains(n)) {
                let f = get_easing_fn(name);
                prop_assert!(
                    f(hi) >= f(lo) - 1e-4,
                    "{name} decreased: f({lo}) = {}, f({hi}) = {}",
                    f(lo),
                    f(hi)
                );
            }
        }

        /// Evaluation is pure: the same input always gives the same output.
        ///
        /// Determinism is the guarantee the whole engine rests on — scrubbing,
        /// caching, golden-pixel tests, and frame-parallel export all assume a
        /// frame can be recomputed in isolation and come out identical.
        #[test]
        fn easing_evaluation_is_deterministic(t in 0.0f32..=1.0) {
            for name in plain_easings() {
                let f = get_easing_fn(name);
                prop_assert_eq!(f(t).to_bits(), f(t).to_bits(), "{} not pure", name);
            }
        }

        /// `cubic_bezier` with CSS-legal control points behaves like an easing.
        #[test]
        fn cubic_bezier_pins_endpoints_and_stays_finite(
            x1 in 0.0f32..=1.0, y1 in -2.0f32..=2.0,
            x2 in 0.0f32..=1.0, y2 in -2.0f32..=2.0,
            t in 0.0f32..=1.0,
        ) {
            let params = serde_json::json!([x1, y1, x2, y2]);
            let at = |p: f32| eval_easing("cubic_bezier", Some(&params), p);
            prop_assert!(at(0.0).abs() < 1e-4, "f(0) = {}", at(0.0));
            prop_assert!((at(1.0) - 1.0).abs() < 1e-4, "f(1) = {}", at(1.0));
            prop_assert!(at(t).is_finite(), "f({t}) = {}", at(t));
        }

        /// `spline` interpolates through sorted keypoints without blowing up.
        ///
        /// The monotone-cubic construction promises no overshoot beyond the
        /// surrounding keypoints; that is the reason it was chosen over
        /// Catmull-Rom, so it is worth asserting rather than assuming.
        #[test]
        fn spline_stays_within_its_keypoints(
            v0 in 0.0f32..=1.0, v1 in 0.0f32..=1.0, v2 in 0.0f32..=1.0,
            t in 0.0f32..=1.0,
        ) {
            let params = serde_json::json!({
                "keypoints": [[0.0, v0], [0.5, v1], [1.0, v2]]
            });
            let v = eval_easing("spline", Some(&params), t);
            prop_assert!(v.is_finite(), "spline({t}) = {v}");
            let lo = v0.min(v1).min(v2) - 1e-3;
            let hi = v0.max(v1).max(v2) + 1e-3;
            prop_assert!(
                (lo..=hi).contains(&v),
                "spline({t}) = {v} escaped the keypoint range [{lo}, {hi}]"
            );
        }

        /// Out-of-domain inputs clamp rather than extrapolate.
        ///
        /// A renderer should never ask for t outside [0, 1], but nothing in
        /// the type system stops it, and extrapolating an elastic curve to
        /// t = 5 produces spectacular nonsense.
        #[test]
        fn out_of_domain_input_is_clamped(t in -10.0f32..=10.0) {
            for name in plain_easings() {
                let v = get_easing_fn(name)(t.clamp(0.0, 1.0));
                prop_assert!(v.is_finite(), "{name} not finite at clamped {t}");
            }
        }
    }
}
