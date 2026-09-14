#[cfg(test)]
#[allow(clippy::module_inception)]
mod interp_tests {
    use lumina_schema::{Canvas, Meta, Scene, TimelineEntry};
    use serde_json::json;

    #[test]
    fn test_linear_interpolation_determinism() {
        let mut objects = std::collections::HashMap::new();
        objects.insert(
            "c1".into(),
            lumina_schema::Object::Circle(lumina_schema::CircleProps {
                cx: 0.0,
                cy: 0.0,
                radius: 10.0,
                z_index: 0,
                fill: "#FFF".into(),
                stroke: None,
                stroke_width: 1.0,
                shadow: None,
                opacity: 1.0,
            }),
        );

        let scene = Scene {
            version: "1.0".into(),
            meta: Meta {
                title: "Test".into(),
                author: "Dev".into(),
                created_at: "now".into(),
            },
            canvas: Canvas {
                width: 100,
                height: 100,
                fps: 60,
                duration: 1.0,
                background: "#000".into(),
                motion_blur_samples: 1,
                shutter: 0.5,
            },
            assets: Default::default(),
            objects,
            timeline: vec![TimelineEntry {
                time: 1.0,
                object: "c1".into(),
                state: json!({"cx": 100.0}),
                easing: "linear".into(),
                easing_params: None,
            }],
            events: vec![],
            camera: None,
        };

        let timeline = crate::timeline::Timeline::from_scene(&scene);
        let state = timeline.get_state_at(0.5);
        let cx = state["c1"]["cx"].as_f64().unwrap();

        assert_eq!(cx, 50.0, "Expected cx=50.0 at t=0.5, got {cx}");
    }

    #[test]
    fn test_color_lab_interpolation_midpoint() {
        use crate::interpolator::interpolate_value;
        use serde_json::Value;

        // Interpolating black (#000000) to white (#FFFFFF) at t=0.5 should give a mid-gray
        let black = Value::String("#000000".to_string());
        let white = Value::String("#FFFFFF".to_string());
        let mid = interpolate_value(&black, &white, 0.5, "linear", None);
        let mid_hex = mid.as_str().expect("Expected string color");

        // Parse the resulting hex and verify it's roughly gray (R≈G≈B≈128±20)
        let hex = mid_hex.trim_start_matches('#');
        assert_eq!(hex.len(), 6, "Expected #RRGGBB format, got: {mid_hex}");
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap();
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap();
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap();
        assert!(r > 80 && r < 200, "Red channel {r} should be mid-range");
        assert_eq!(r, g, "Mid-gray should have equal R/G/B channels");
        assert_eq!(g, b, "Mid-gray should have equal R/G/B channels");
    }

    #[test]
    fn test_color_interpolation_at_t0_returns_start() {
        use crate::interpolator::interpolate_value;
        use serde_json::Value;

        let red = Value::String("#FF0000".to_string());
        let blue = Value::String("#0000FF".to_string());
        let result = interpolate_value(&red, &blue, 0.0, "linear", None);
        assert_eq!(result.as_str().unwrap(), "#FF0000");
    }

    #[test]
    fn test_color_interpolation_at_t1_returns_end() {
        use crate::interpolator::interpolate_value;
        use serde_json::Value;

        let red = Value::String("#FF0000".to_string());
        let blue = Value::String("#0000FF".to_string());
        let result = interpolate_value(&red, &blue, 1.0, "linear", None);
        assert_eq!(result.as_str().unwrap(), "#0000FF");
    }

    #[test]
    fn test_non_color_strings_snap_to_v2() {
        use crate::interpolator::interpolate_value;
        use serde_json::Value;

        let v1 = Value::String("hello".to_string());
        let v2 = Value::String("world".to_string());
        let result = interpolate_value(&v1, &v2, 0.5, "linear", None);
        assert_eq!(result.as_str().unwrap(), "world");
    }

    #[test]
    fn test_path_morph_same_length() {
        use crate::interpolator::interpolate_value;
        use serde_json::json;

        let a = json!([[0.0, 0.0], [10.0, 0.0]]);
        let b = json!([[0.0, 10.0], [10.0, 10.0]]);
        let mid = interpolate_value(&a, &b, 0.5, "linear", None);
        let arr = mid.as_array().unwrap();
        assert_eq!(arr.len(), 2, "morphed array should have 2 points");
        let pt0 = arr[0].as_array().unwrap();
        let y0 = pt0[1].as_f64().unwrap() as f32;
        assert!(
            (y0 - 5.0).abs() < 0.01,
            "y should be 5.0 at t=0.5, got {y0}"
        );
    }

    #[test]
    fn test_path_morph_unequal_lengths() {
        use crate::interpolator::interpolate_value;
        use serde_json::json;

        let a = json!([[0.0, 0.0], [10.0, 0.0]]);
        let b = json!([[0.0, 10.0], [5.0, 10.0], [10.0, 10.0]]);
        let mid = interpolate_value(&a, &b, 0.5, "linear", None);
        let arr = mid.as_array().unwrap();
        assert_eq!(arr.len(), 3, "result should have max(2,3)=3 points");
    }

    #[test]
    fn test_path_morph_t0_returns_start() {
        use crate::interpolator::interpolate_value;
        use serde_json::json;

        let a = json!([[1.0, 2.0]]);
        let b = json!([[9.0, 8.0]]);
        let result = interpolate_value(&a, &b, 0.0, "linear", None);
        let arr = result.as_array().unwrap();
        let x = arr[0].as_array().unwrap()[0].as_f64().unwrap() as f32;
        assert!(
            (x - 1.0).abs() < 0.001,
            "at t=0 should be start value, got x={x}"
        );
    }

    #[test]
    fn test_cubic_bezier_interpolation_via_timeline() {
        let mut objects = std::collections::HashMap::new();
        objects.insert(
            "c1".into(),
            lumina_schema::Object::Circle(lumina_schema::CircleProps {
                cx: 0.0,
                cy: 0.0,
                radius: 10.0,
                z_index: 0,
                fill: "#FFF".into(),
                stroke: None,
                stroke_width: 1.0,
                shadow: None,
                opacity: 1.0,
            }),
        );
        let scene = lumina_schema::Scene {
            version: "1.0".into(),
            meta: lumina_schema::Meta {
                title: "T".into(),
                author: "x".into(),
                created_at: "now".into(),
            },
            canvas: lumina_schema::Canvas {
                width: 100,
                height: 100,
                fps: 30,
                duration: 2.0,
                background: "#000".into(),
                motion_blur_samples: 1,
                shutter: 0.5,
            },
            assets: Default::default(),
            objects,
            timeline: vec![lumina_schema::TimelineEntry {
                time: 2.0,
                object: "c1".into(),
                state: json!({"cx": 100.0}),
                easing: "cubic_bezier".into(),
                easing_params: Some(json!([0.0, 0.0, 1.0, 1.0])),
            }],
            events: vec![],
            camera: None,
        };
        let timeline = crate::timeline::Timeline::from_scene(&scene);
        let state = timeline.get_state_at(1.0);
        let cx = state["c1"]["cx"].as_f64().unwrap() as f32;
        // cubic_bezier(0,0,1,1) is linear, so cx at t=0.5 should be ~50
        assert!(
            (cx - 50.0).abs() < 5.0,
            "cubic_bezier linear at midpoint: cx={cx}"
        );
    }
}

/// Morphing between shapes with different vertex counts.
///
/// Padding the shorter list with its last element is a correct definition and
/// wrong motion: a four-point square becoming a sixty-four-point circle mapped
/// sixty-one of the circle's vertices onto one corner of the square, so the
/// shape collapsed into that corner and unfolded instead of flowing.
#[cfg(test)]
mod morphing {
    use crate::interpolator::interpolate_value;
    use serde_json::{json, Value};

    fn square() -> Value {
        json!([[0.0, 0.0], [100.0, 0.0], [100.0, 100.0], [0.0, 100.0]])
    }

    fn circle(n: usize) -> Value {
        let pts: Vec<Value> = (0..n)
            .map(|i| {
                let a = std::f64::consts::TAU * i as f64 / n as f64;
                json!([50.0 + 50.0 * a.cos(), 50.0 + 50.0 * a.sin()])
            })
            .collect();
        Value::Array(pts)
    }

    fn points(v: &Value) -> Vec<(f64, f64)> {
        v.as_array()
            .expect("array")
            .iter()
            .map(|p| {
                let a = p.as_array().expect("point");
                (a[0].as_f64().expect("x"), a[1].as_f64().expect("y"))
            })
            .collect()
    }

    #[test]
    fn a_morph_does_not_bunch_vertices() {
        // The actual defect. Halfway through, the vertices should be spread
        // around the shape; padding piled most of them onto one point.
        let mid = interpolate_value(&square(), &circle(64), 0.5, "linear", None);
        let pts = points(&mid);
        assert_eq!(pts.len(), 64);

        // No single location may hold a large share of the vertices.
        let mut worst = 0;
        for (i, a) in pts.iter().enumerate() {
            let near = pts
                .iter()
                .enumerate()
                .filter(|(j, b)| *j != i && (b.0 - a.0).hypot(b.1 - a.1) < 2.0)
                .count();
            worst = worst.max(near);
        }
        assert!(
            worst < 8,
            "{worst} vertices coincide — the morph is collapsing rather than flowing"
        );
    }

    #[test]
    fn the_endpoints_are_the_original_shapes() {
        // Resampling must not disturb where the morph starts and ends. The
        // resampled square is still a square: its vertices lie on the outline.
        let start = points(&interpolate_value(
            &square(),
            &circle(64),
            0.0,
            "linear",
            None,
        ));
        for (x, y) in &start {
            let on_edge = (x.abs() < 0.01 || (x - 100.0).abs() < 0.01)
                || (y.abs() < 0.01 || (y - 100.0).abs() < 0.01);
            assert!(on_edge, "({x}, {y}) is not on the square's outline");
        }

        let end = points(&interpolate_value(
            &square(),
            &circle(64),
            1.0,
            "linear",
            None,
        ));
        for (x, y) in &end {
            let r = ((x - 50.0).powi(2) + (y - 50.0).powi(2)).sqrt();
            assert!((r - 50.0).abs() < 0.5, "({x}, {y}) is not on the circle");
        }
    }

    #[test]
    fn equal_length_lists_keep_their_pairwise_correspondence() {
        // When the author gives matching counts they mean vertex i to vertex i,
        // and resampling would silently override that.
        let a = json!([[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]]);
        let b = json!([[100.0, 100.0], [110.0, 100.0], [110.0, 110.0]]);
        let mid = points(&interpolate_value(&a, &b, 0.5, "linear", None));
        assert_eq!(mid[0], (50.0, 50.0));
        assert_eq!(mid[1], (60.0, 50.0));
        assert_eq!(mid[2], (60.0, 60.0));
    }

    #[test]
    fn non_point_arrays_still_pad() {
        // A gradient-stop list and a bezier parameter array are arrays too, and
        // resampling either by "arc length" would be nonsense.
        let stops_a = json!([[0.0, "#FF0000"], [1.0, "#0000FF"]]);
        let stops_b = json!([[0.0, "#00FF00"], [0.5, "#FFFF00"], [1.0, "#FF00FF"]]);
        let out = interpolate_value(&stops_a, &stops_b, 0.5, "linear", None);
        assert_eq!(
            out.as_array().expect("array").len(),
            3,
            "padded, not resampled"
        );

        let flat_a = json!([0.0, 0.0, 1.0]);
        let flat_b = json!([0.0, 0.0, 1.0, 1.0]);
        let out = interpolate_value(&flat_a, &flat_b, 0.5, "linear", None);
        assert_eq!(out.as_array().expect("array").len(), 4);
    }

    #[test]
    fn degenerate_shapes_do_not_panic() {
        let single = json!([[5.0, 5.0], [5.0, 5.0]]);
        for t in [0.0f32, 0.5, 1.0] {
            let _ = interpolate_value(&single, &circle(16), t, "linear", None);
            let _ = interpolate_value(&circle(16), &single, t, "linear", None);
        }
    }
}
