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
}
