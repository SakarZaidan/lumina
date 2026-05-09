#[cfg(test)]
mod interp_tests {
    use super::*;
    use lumina_schema::{Scene, Canvas, Meta, TimelineEntry};
    use serde_json::json;

    #[test]
    fn test_linear_interpolation_determinism() {
        let mut objects = std::collections::HashMap::new();
        objects.insert("c1".into(), lumina_schema::Object::Circle(lumina_schema::CircleProps {
            cx: 0.0, cy: 0.0, radius: 10.0, z_index: 0, fill: "#FFF".into(), stroke: None, stroke_width: 1.0, opacity: 1.0
        }));
        
        let scene = Scene {
            version: "1.0".into(),
            meta: Meta { title: "Test".into(), author: "Dev".into(), created_at: "now".into() },
            canvas: Canvas { width: 100, height: 100, fps: 60, duration: 1.0, background: "#000".into() },
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
        
        assert_eq!(cx, 50.0, "Expected cx=50.0 at t=0.5, got {}", cx);
    }

    #[test]
    fn test_color_lab_interpolation_midpoint() {
        use crate::interpolator::interpolate_value;
        use serde_json::Value;

        // Interpolating black (#000000) to white (#FFFFFF) at t=0.5 should give a mid-gray
        let black = Value::String("#000000".to_string());
        let white = Value::String("#FFFFFF".to_string());
        let mid = interpolate_value(&black, &white, 0.5, "linear");
        let mid_hex = mid.as_str().expect("Expected string color");

        // Parse the resulting hex and verify it's roughly gray (R≈G≈B≈128±20)
        let hex = mid_hex.trim_start_matches('#');
        assert_eq!(hex.len(), 6, "Expected #RRGGBB format, got: {}", mid_hex);
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
        let result = interpolate_value(&red, &blue, 0.0, "linear");
        assert_eq!(result.as_str().unwrap(), "#FF0000");
    }

    #[test]
    fn test_color_interpolation_at_t1_returns_end() {
        use crate::interpolator::interpolate_value;
        use serde_json::Value;

        let red = Value::String("#FF0000".to_string());
        let blue = Value::String("#0000FF".to_string());
        let result = interpolate_value(&red, &blue, 1.0, "linear");
        assert_eq!(result.as_str().unwrap(), "#0000FF");
    }

    #[test]
    fn test_non_color_strings_snap_to_v2() {
        use crate::interpolator::interpolate_value;
        use serde_json::Value;

        // Non-hex strings should still snap to v2 (no interpolation)
        let v1 = Value::String("hello".to_string());
        let v2 = Value::String("world".to_string());
        let result = interpolate_value(&v1, &v2, 0.5, "linear");
        assert_eq!(result.as_str().unwrap(), "world");
    }
}
