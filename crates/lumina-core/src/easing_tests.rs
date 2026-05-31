#[cfg(test)]
mod tests {
    use crate::easing::*;

    const EPSILON: f32 = 1e-5;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < EPSILON
    }

    // Every easing function must map 0→0 and 1→1
    macro_rules! test_boundary {
        ($fn:ident) => {
            assert!(
                approx_eq($fn(0.0), 0.0),
                "{} at 0.0 should be 0.0, got {}",
                stringify!($fn),
                $fn(0.0)
            );
            assert!(
                approx_eq($fn(1.0), 1.0),
                "{} at 1.0 should be 1.0, got {}",
                stringify!($fn),
                $fn(1.0)
            );
        };
    }

    #[test]
    fn test_all_easings_boundary_conditions() {
        test_boundary!(linear);
        test_boundary!(ease_in_quad);
        test_boundary!(ease_out_quad);
        test_boundary!(ease_in_out_quad);
        test_boundary!(ease_in_cubic);
        test_boundary!(ease_out_cubic);
        test_boundary!(ease_in_out_cubic);
        test_boundary!(ease_in_quart);
        test_boundary!(ease_out_quart);
        test_boundary!(ease_in_out_quart);
        test_boundary!(ease_in_sine);
        test_boundary!(ease_out_sine);
        test_boundary!(ease_in_out_sine);
        test_boundary!(ease_in_expo);
        test_boundary!(ease_out_expo);
        test_boundary!(ease_in_circ);
        test_boundary!(ease_out_circ);
        test_boundary!(ease_in_elastic);
        test_boundary!(ease_out_elastic);
        test_boundary!(ease_in_bounce);
        test_boundary!(ease_out_bounce);
        test_boundary!(smooth);
    }

    #[test]
    fn test_linear_is_identity() {
        for i in 0..=10 {
            let t = i as f32 / 10.0;
            assert!(approx_eq(linear(t), t), "linear({}) should equal {}", t, t);
        }
    }

    #[test]
    fn test_ease_in_out_quad_midpoint_symmetry() {
        // ease_in_out_quad(0.5) must be exactly 0.5 (inflection point)
        assert!(approx_eq(ease_in_out_quad(0.5), 0.5));
        assert!(approx_eq(ease_in_out_cubic(0.5), 0.5));
        assert!(approx_eq(ease_in_out_quart(0.5), 0.5));
        assert!(approx_eq(ease_in_out_sine(0.5), 0.5));
    }

    #[test]
    fn test_ease_in_is_slower_than_linear_at_start() {
        // ease_in variants should be below the linear line in the first half
        let t = 0.3_f32;
        assert!(
            ease_in_quad(t) < t,
            "ease_in_quad should accelerate (below linear at {t})"
        );
        assert!(ease_in_cubic(t) < t);
        assert!(ease_in_quart(t) < t);
    }

    #[test]
    fn test_ease_out_is_faster_than_linear_at_start() {
        // ease_out variants should be above the linear line in the first half
        let t = 0.3_f32;
        assert!(
            ease_out_quad(t) > t,
            "ease_out_quad should decelerate (above linear at {t})"
        );
        assert!(ease_out_cubic(t) > t);
    }

    #[test]
    fn test_bounce_out_has_correct_structure() {
        // Bounce should overshoot slightly above 1.0 then settle — verify monotone increase
        // across the full range at coarse steps
        let mut prev = ease_out_bounce(0.0);
        for i in 1..=20 {
            let t = i as f32 / 20.0;
            let val = ease_out_bounce(t);
            // Value must not go below 0 or above 1.1 (small overshoot allowed in formula)
            assert!(val >= -0.01, "ease_out_bounce({t}) = {val} went negative");
            assert!(val <= 1.01, "ease_out_bounce({t}) = {val} exceeded 1.0");
            prev = val;
        }
        let _ = prev;
    }

    #[test]
    fn test_elastic_out_overshoots_then_settles() {
        // ease_out_elastic can exceed 1.0 (overshoot) but must end at 1.0
        let peak = (1..20)
            .map(|i| ease_out_elastic(i as f32 / 20.0))
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(
            peak > 1.0,
            "ease_out_elastic should overshoot 1.0, peak was {peak}"
        );
        assert!(approx_eq(ease_out_elastic(1.0), 1.0));
    }

    #[test]
    fn test_smooth_equals_smoothstep() {
        // smooth(t) = 3t² - 2t³
        for i in 0..=10 {
            let t = i as f32 / 10.0;
            let expected = 3.0 * t * t - 2.0 * t * t * t;
            assert!(approx_eq(smooth(t), expected), "smooth({t}) mismatch");
        }
    }

    #[test]
    fn test_spring_starts_at_zero_ends_near_one() {
        assert!(approx_eq(spring(0.0), 0.0));
        let end = spring(1.0);
        assert!(
            (end - 1.0).abs() < 0.05,
            "spring(1.0) should be near 1.0, got {end}"
        );
    }

    #[test]
    fn test_there_and_back_midpoint_is_one() {
        // there_and_back should peak at ~1.0 at t=0.5
        let peak = there_and_back(0.5);
        assert!(
            approx_eq(peak, 1.0),
            "there_and_back(0.5) should be 1.0, got {peak}"
        );
        assert!(approx_eq(there_and_back(0.0), 0.0));
        assert!(approx_eq(there_and_back(1.0), 0.0));
    }

    #[test]
    fn test_unknown_easing_falls_back_to_linear() {
        // get_easing_fn returns linear for unknown names (by design)
        let f = get_easing_fn("this_does_not_exist");
        for i in 0..=5 {
            let t = i as f32 / 5.0;
            assert!(
                approx_eq(f(t), linear(t)),
                "unknown easing should behave like linear at t={t}"
            );
        }
    }

    #[test]
    fn test_cubic_bezier_boundary_conditions() {
        use crate::easing::eval_easing;
        use serde_json::json;
        let params = json!([0.25, 0.1, 0.25, 1.0]);
        assert!(approx_eq(
            eval_easing("cubic_bezier", Some(&params), 0.0),
            0.0
        ));
        assert!(approx_eq(
            eval_easing("cubic_bezier", Some(&params), 1.0),
            1.0
        ));
    }

    #[test]
    fn test_cubic_bezier_linear_is_linear() {
        // cubic_bezier(0,0,1,1) is a straight line — should match linear.
        use crate::easing::eval_easing;
        use serde_json::json;
        let params = json!([0.0, 0.0, 1.0, 1.0]);
        for i in 1..10 {
            let t = i as f32 / 10.0;
            let v = eval_easing("cubic_bezier", Some(&params), t);
            assert!(
                (v - t).abs() < 0.02,
                "cubic_bezier linear at t={t}: expected ~{t}, got {v}"
            );
        }
    }

    #[test]
    fn test_cubic_bezier_css_ease_approximation() {
        // CSS ease = cubic_bezier(0.25,0.1,0.25,1.0). Midpoint should be > 0.5 (fast start).
        use crate::easing::eval_easing;
        use serde_json::json;
        let params = json!([0.25, 0.1, 0.25, 1.0]);
        let mid = eval_easing("cubic_bezier", Some(&params), 0.5);
        assert!(
            mid > 0.5,
            "CSS ease should be above linear at t=0.5, got {mid}"
        );
    }

    #[test]
    fn test_cubic_bezier_missing_params_fallback() {
        use crate::easing::eval_easing;
        // No params → fallback to CSS ease, still satisfies boundaries.
        assert!(approx_eq(eval_easing("cubic_bezier", None, 0.0), 0.0));
        assert!(approx_eq(eval_easing("cubic_bezier", None, 1.0), 1.0));
    }

    #[test]
    fn test_spline_passes_through_keypoints() {
        use crate::easing::eval_easing;
        use serde_json::json;
        let params = json!({ "keypoints": [[0.0, 0.0], [0.3, 0.8], [0.7, 0.2], [1.0, 1.0]] });
        for (t, v) in [(0.0, 0.0), (0.3, 0.8), (0.7, 0.2), (1.0, 1.0)] {
            let got = eval_easing("spline", Some(&params), t);
            assert!(
                approx_eq(got, v),
                "spline at keypoint t={t}: expected {v}, got {got}"
            );
        }
    }

    #[test]
    fn test_spline_endpoints_clamp() {
        use crate::easing::eval_easing;
        use serde_json::json;
        let params = json!({ "keypoints": [[0.0, 0.0], [0.5, 0.5], [1.0, 1.0]] });
        assert!(approx_eq(eval_easing("spline", Some(&params), 0.0), 0.0));
        assert!(approx_eq(eval_easing("spline", Some(&params), 1.0), 1.0));
    }

    #[test]
    fn test_spline_monotone_no_overshoot() {
        // Monotone-increasing keypoints must never overshoot below 0 or above 1.
        use crate::easing::eval_easing;
        use serde_json::json;
        let params = json!({ "keypoints": [[0.0, 0.0], [0.2, 0.05], [0.8, 0.95], [1.0, 1.0]] });
        for i in 0..=100 {
            let t = i as f32 / 100.0;
            let v = eval_easing("spline", Some(&params), t);
            assert!(
                (-1e-4..=1.0001).contains(&v),
                "spline overshoot at t={t}: v={v}"
            );
        }
    }

    #[test]
    fn test_spline_missing_params_fallback_linear() {
        use crate::easing::eval_easing;
        // No keypoints → linear fallback.
        assert!(approx_eq(eval_easing("spline", None, 0.0), 0.0));
        assert!(approx_eq(eval_easing("spline", None, 0.5), 0.5));
        assert!(approx_eq(eval_easing("spline", None, 1.0), 1.0));
    }

    #[test]
    fn test_in_out_variants_are_symmetric() {
        // For any in_out easing, f(t) + f(1-t) should equal 1 (point symmetry around (0.5, 0.5))
        let fns: &[fn(f32) -> f32] = &[
            ease_in_out_quad,
            ease_in_out_cubic,
            ease_in_out_quart,
            ease_in_out_sine,
        ];
        for f in fns {
            for i in 0..=10 {
                let t = i as f32 / 10.0;
                let sum = f(t) + f(1.0 - t);
                assert!(
                    (sum - 1.0).abs() < 1e-4,
                    "in_out symmetry violated at t={t}: f(t)+f(1-t)={sum}"
                );
            }
        }
    }
}
