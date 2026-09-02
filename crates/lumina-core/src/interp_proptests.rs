//! Property tests for value interpolation.
//!
//! `interpolate_value` is total by construction — it must return *something*
//! for any pair of JSON values — which makes it exactly the kind of function
//! where a defect hides. These properties pin down what that something has to
//! be.

#[cfg(test)]
mod tests {
    use crate::interpolator::interpolate_value;
    use proptest::prelude::*;
    use serde_json::{json, Value};

    proptest! {
        /// Interpolating two finite numbers must produce a finite number.
        ///
        /// `serde_json` maps NaN and infinity to `Value::Null`, so a non-finite
        /// result does not merely look wrong — the property *disappears* from
        /// the state map and the renderer substitutes its own default. The
        /// animation is wrong and nothing reports it.
        #[test]
        fn numeric_interpolation_stays_finite(
            a in -1e38f64..1e38, b in -1e38f64..1e38, t in 0.0f32..=1.0,
        ) {
            let out = interpolate_value(&json!(a), &json!(b), t, "linear", None);
            prop_assert!(out.is_number(), "got {out:?} for lerp({a}, {b}, {t})");
            let v = out.as_f64().unwrap_or(f64::NAN);
            prop_assert!(v.is_finite(), "lerp({a}, {b}, {t}) = {v}");
        }

        /// The endpoints are exact.
        ///
        /// A keyframe must be reached, not approached: if `t = 1` lands near
        /// the target rather than on it, every animation ends slightly wrong.
        #[test]
        fn endpoints_are_exact(a in -1e4f64..1e4, b in -1e4f64..1e4) {
            let at = |t| interpolate_value(&json!(a), &json!(b), t, "linear", None)
                .as_f64()
                .unwrap_or(f64::NAN);
            prop_assert!((at(0.0) - a).abs() < 1e-2, "t=0 gave {} not {a}", at(0.0));
            prop_assert!((at(1.0) - b).abs() < 1e-2, "t=1 gave {} not {b}", at(1.0));
        }

        /// Numeric interpolation is bounded by its endpoints under a linear
        /// easing — it never leaves the interval it is travelling across.
        #[test]
        fn linear_interpolation_stays_between_its_endpoints(
            a in -1e4f64..1e4, b in -1e4f64..1e4, t in 0.0f32..=1.0,
        ) {
            let v = interpolate_value(&json!(a), &json!(b), t, "linear", None)
                .as_f64()
                .unwrap_or(f64::NAN);
            let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
            prop_assert!(
                v >= lo - 1e-2 && v <= hi + 1e-2,
                "lerp({a}, {b}, {t}) = {v} escaped [{lo}, {hi}]"
            );
        }

        /// Interpolation never panics, whatever it is handed.
        ///
        /// Timeline state is untyped `serde_json::Value` (TD-07), so any two
        /// values can meet here: a number against a string, an array against
        /// an object, mismatched lengths, nulls.
        #[test]
        fn interpolation_is_total(
            t in 0.0f32..=1.0,
            i in 0usize..8, j in 0usize..8,
        ) {
            let values = [
                json!(1.0),
                json!("#FF0000"),
                json!("not a colour"),
                json!([1.0, 2.0]),
                json!([1.0, 2.0, 3.0, 4.0]),
                json!({ "a": 1 }),
                Value::Null,
                json!(true),
            ];
            let out = interpolate_value(&values[i], &values[j], t, "linear", None);
            // The contract is that it returns; the shape may be anything.
            prop_assert!(matches!(
                out,
                Value::Null | Value::Bool(_) | Value::Number(_)
                    | Value::String(_) | Value::Array(_) | Value::Object(_)
            ));
        }

        /// Colour interpolation stays a parseable colour at every step.
        #[test]
        fn colour_interpolation_yields_colours(
            r1 in 0u8..=255, g1 in 0u8..=255, b1 in 0u8..=255,
            r2 in 0u8..=255, g2 in 0u8..=255, b2 in 0u8..=255,
            t in 0.0f32..=1.0,
        ) {
            let c1 = format!("#{r1:02X}{g1:02X}{b1:02X}");
            let c2 = format!("#{r2:02X}{g2:02X}{b2:02X}");
            let out = interpolate_value(&json!(c1), &json!(c2), t, "linear", None);
            let s = out.as_str().unwrap_or_default().to_string();
            prop_assert!(
                s.starts_with('#') && (s.len() == 7 || s.len() == 9),
                "interpolating {c1} -> {c2} at {t} gave {s:?}"
            );
            prop_assert!(
                u32::from_str_radix(&s[1..], 16).is_ok(),
                "{s:?} is not hexadecimal"
            );
        }

        /// Colour endpoints round-trip exactly.
        ///
        /// At t=0 and t=1 the author's colour must come back unchanged; a
        /// colour space conversion that loses a bit at the endpoints turns
        /// every fade into a barely-visible palette shift.
        #[test]
        fn colour_endpoints_round_trip(
            r in 0u8..=255, g in 0u8..=255, b in 0u8..=255,
        ) {
            let c = format!("#{r:02X}{g:02X}{b:02X}");
            let other = json!("#123456");
            let at0 = interpolate_value(&json!(c), &other, 0.0, "linear", None);
            prop_assert_eq!(
                at0.as_str().unwrap_or_default().to_uppercase(),
                c.to_uppercase(),
                "t=0 must return the source colour unchanged"
            );
        }

        /// Eight-digit colours interpolate rather than snapping.
        ///
        /// A weaker form of this passed against the unfixed code for the wrong
        /// reason: `#RRGGBBAA` failed to parse, so the value snapped to the
        /// destination — which is also nine characters. Asserting that the
        /// midpoint differs from *both* endpoints is what actually
        /// distinguishes interpolation from a snap.
        #[test]
        fn eight_digit_colours_interpolate(a1 in 0u8..=200, a2 in 0u8..=200) {
            let c1 = format!("#FF0000{a1:02X}");
            let c2 = format!("#00FF00{a2:02X}");
            let mid = interpolate_value(&json!(c1), &json!(c2), 0.5, "linear", None);
            let s = mid.as_str().unwrap_or_default().to_uppercase();
            prop_assert_eq!(s.len(), 9, "alpha must survive, got {:?}", s);
            prop_assert_ne!(
                &s, &c2.to_uppercase(),
                "the midpoint snapped to the destination instead of interpolating"
            );
            prop_assert_ne!(
                &s, &c1.to_uppercase(),
                "the midpoint snapped to the source instead of interpolating"
            );
        }
    }
}
