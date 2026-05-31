#[cfg(test)]
mod tests {
    use crate::timeline::Timeline;
    use lumina_schema::{Canvas, CircleProps, Meta, Object, Scene, TimelineEntry};
    use serde_json::json;
    use std::collections::HashMap;

    fn make_scene(objects: HashMap<String, Object>, timeline: Vec<TimelineEntry>) -> Scene {
        Scene {
            version: "1.0".into(),
            meta: Meta {
                title: "T".into(),
                author: "T".into(),
                created_at: "now".into(),
            },
            canvas: Canvas {
                width: 100,
                height: 100,
                fps: 60,
                duration: 10.0,
                background: "#000".into(),
            },
            assets: Default::default(),
            objects,
            timeline,
            events: vec![],
            camera: None,
        }
    }

    fn circle(cx: f32, opacity: f32) -> Object {
        Object::Circle(CircleProps {
            cx,
            cy: 0.0,
            radius: 10.0,
            z_index: 0,
            fill: "#FFF".into(),
            stroke: None,
            stroke_width: 0.0,
            shadow: None,
            opacity,
        })
    }

    #[test]
    fn test_initial_property_seeded_at_t0() {
        let mut objs = HashMap::new();
        objs.insert("c".into(), circle(100.0, 0.5));
        let scene = make_scene(objs, vec![]);

        let tl = Timeline::from_scene(&scene);
        let state = tl.get_state_at(0.0);

        let opacity = state["c"]["opacity"].as_f64().unwrap();
        assert!(
            (opacity - 0.5).abs() < 1e-5,
            "Expected opacity=0.5 at t=0, got {opacity}"
        );
    }

    #[test]
    fn test_state_at_exact_keyframe_time() {
        let mut objs = HashMap::new();
        objs.insert("c".into(), circle(0.0, 0.0));
        let kf = TimelineEntry {
            time: 2.0,
            object: "c".into(),
            state: json!({"opacity": 1.0}),
            easing: "linear".into(),
            easing_params: None,
        };
        let scene = make_scene(objs, vec![kf]);
        let tl = Timeline::from_scene(&scene);

        let state = tl.get_state_at(2.0);
        let opacity = state["c"]["opacity"].as_f64().unwrap();
        assert!(
            (opacity - 1.0).abs() < 1e-5,
            "Expected opacity=1.0 at t=2.0, got {opacity}"
        );
    }

    #[test]
    fn test_linear_interpolation_at_midpoint() {
        let mut objs = HashMap::new();
        objs.insert("c".into(), circle(0.0, 0.0));
        let kf = TimelineEntry {
            time: 2.0,
            object: "c".into(),
            state: json!({"opacity": 1.0}),
            easing: "linear".into(),
            easing_params: None,
        };
        let scene = make_scene(objs, vec![kf]);
        let tl = Timeline::from_scene(&scene);

        let state = tl.get_state_at(1.0); // halfway between t=0 and t=2
        let opacity = state["c"]["opacity"].as_f64().unwrap();
        assert!(
            (opacity - 0.5).abs() < 1e-4,
            "Expected opacity=0.5 at t=1.0, got {opacity}"
        );
    }

    #[test]
    fn test_clamp_before_first_keyframe() {
        let mut objs = HashMap::new();
        objs.insert("c".into(), circle(0.0, 0.25));
        let scene = make_scene(objs, vec![]);
        let tl = Timeline::from_scene(&scene);

        let state = tl.get_state_at(-5.0);
        let opacity = state["c"]["opacity"].as_f64().unwrap();
        assert!(
            (opacity - 0.25).abs() < 1e-5,
            "Should clamp to first keyframe value before start"
        );
    }

    #[test]
    fn test_clamp_after_last_keyframe() {
        let mut objs = HashMap::new();
        objs.insert("c".into(), circle(0.0, 0.0));
        let kf = TimelineEntry {
            time: 1.0,
            object: "c".into(),
            state: json!({"opacity": 0.9}),
            easing: "linear".into(),
            easing_params: None,
        };
        let scene = make_scene(objs, vec![kf]);
        let tl = Timeline::from_scene(&scene);

        let state = tl.get_state_at(999.0);
        let opacity = state["c"]["opacity"].as_f64().unwrap();
        assert!(
            (opacity - 0.9).abs() < 1e-5,
            "Should clamp to last keyframe value after end"
        );
    }

    #[test]
    fn test_override_takes_precedence_over_keyframe() {
        let mut objs = HashMap::new();
        objs.insert("c".into(), circle(0.0, 0.0));
        let kf = TimelineEntry {
            time: 1.0,
            object: "c".into(),
            state: json!({"opacity": 1.0}),
            easing: "linear".into(),
            easing_params: None,
        };
        let scene = make_scene(objs, vec![kf]);
        let mut tl = Timeline::from_scene(&scene);

        tl.override_property("c", "opacity", json!(0.42));
        let state = tl.get_state_at(0.5);
        let opacity = state["c"]["opacity"].as_f64().unwrap();
        assert!(
            (opacity - 0.42).abs() < 1e-5,
            "Override should take precedence, got {opacity}"
        );
    }

    #[test]
    fn test_ease_in_quad_is_nonlinear_at_midpoint() {
        let mut objs = HashMap::new();
        objs.insert("c".into(), circle(0.0, 0.0));
        let kf = TimelineEntry {
            time: 2.0,
            object: "c".into(),
            state: json!({"opacity": 1.0}),
            easing: "ease_in_quad".into(),
            easing_params: None,
        };
        let scene = make_scene(objs, vec![kf]);
        let tl = Timeline::from_scene(&scene);

        let state = tl.get_state_at(1.0);
        let opacity = state["c"]["opacity"].as_f64().unwrap() as f32;
        // ease_in_quad(0.5) = 0.25, not 0.5 — confirms non-linear behavior
        assert!(
            (opacity - 0.25).abs() < 1e-3,
            "ease_in_quad at midpoint should be ~0.25, got {opacity}"
        );
    }

    #[test]
    fn test_two_objects_do_not_bleed_values() {
        let mut objs = HashMap::new();
        objs.insert("a".into(), circle(0.0, 1.0));
        objs.insert("b".into(), circle(0.0, 0.0));
        let kf_a = TimelineEntry {
            time: 1.0,
            object: "a".into(),
            state: json!({"cx": 100.0}),
            easing: "linear".into(),
            easing_params: None,
        };
        let kf_b = TimelineEntry {
            time: 1.0,
            object: "b".into(),
            state: json!({"cx": 200.0}),
            easing: "linear".into(),
            easing_params: None,
        };
        let scene = make_scene(objs, vec![kf_a, kf_b]);
        let tl = Timeline::from_scene(&scene);

        let state = tl.get_state_at(0.5);
        let cx_a = state["a"]["cx"].as_f64().unwrap();
        let cx_b = state["b"]["cx"].as_f64().unwrap();
        assert!(
            (cx_a - 50.0).abs() < 1e-3,
            "Object 'a' cx should be ~50, got {cx_a}"
        );
        assert!(
            (cx_b - 100.0).abs() < 1e-3,
            "Object 'b' cx should be ~100, got {cx_b}"
        );
    }

    #[test]
    fn test_multiple_properties_interpolate_independently() {
        let mut objs = HashMap::new();
        objs.insert("c".into(), circle(0.0, 0.0));
        let scene = make_scene(
            objs,
            vec![TimelineEntry {
                time: 2.0,
                object: "c".into(),
                state: json!({"cx": 100.0, "opacity": 1.0}),
                easing: "linear".into(),
                easing_params: None,
            }],
        );
        let tl = Timeline::from_scene(&scene);
        let state = tl.get_state_at(1.0);

        let cx = state["c"]["cx"].as_f64().unwrap();
        let opacity = state["c"]["opacity"].as_f64().unwrap();
        assert!((cx - 50.0).abs() < 1e-3);
        assert!((opacity - 0.5).abs() < 1e-3);
    }
}
