#[cfg(test)]
mod tests {
    use crate::timeline::Timeline;
    use luminafx_schema::{Canvas, CircleProps, Meta, Object, Scene, TimelineEntry};
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
                motion_blur_samples: 1,
                shutter: 0.5,
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

/// A camera keyframe must follow the curve it names, like any other property.
///
/// `CameraTimelineEntry` had no `easing_params` field, so `cubic_bezier` and
/// `spline` passed validation — both are registered easing names — and then
/// animated **linearly**, because the parameterless lookup does not know them.
/// Camera moves are the most visible motion in a scene, which makes it a bad
/// place for a silent fallback.
#[cfg(test)]
mod camera_easing {
    use crate::Timeline;
    use luminafx_schema::{Camera, CameraState, CameraTimelineEntry, Scene};

    fn scene_with_camera(easing: &str, params: Option<serde_json::Value>) -> Scene {
        let mut scene: Scene = serde_json::from_value(serde_json::json!({
            "version": "1.0",
            "meta": { "title": "t", "author": "a", "created_at": "2026-01-01T00:00:00Z" },
            "canvas": { "width": 64, "height": 64, "fps": 30, "duration": 2.0,
                        "background": "#000000" },
            "objects": {},
            "timeline": []
        }))
        .expect("fixture");
        scene.camera = Some(Camera {
            timeline: vec![
                CameraTimelineEntry {
                    time: 0.0,
                    state: CameraState {
                        x: 0.0,
                        y: 0.0,
                        zoom: 1.0,
                        rotation: 0.0,
                    },
                    easing: "linear".into(),
                    easing_params: None,
                },
                CameraTimelineEntry {
                    time: 1.0,
                    state: CameraState {
                        x: 100.0,
                        y: 0.0,
                        zoom: 1.0,
                        rotation: 0.0,
                    },
                    easing: easing.into(),
                    easing_params: params,
                },
            ],
        });
        scene
    }

    #[test]
    fn a_parameterised_camera_easing_follows_its_curve() {
        // The same parameters applied to an object property, so the two must
        // agree — that is the whole claim.
        let params = serde_json::json!([0.9, 0.0, 0.9, 1.0]);
        let scene = scene_with_camera("cubic_bezier", Some(params.clone()));
        let timeline = Timeline::from_scene(&scene);

        for step in 1..10 {
            let t = f64::from(step) / 10.0;
            let expected =
                crate::easing::eval_easing("cubic_bezier", Some(&params), t as f32) * 100.0;
            let actual = timeline.get_camera_at(t as f32, &scene).x;
            assert!(
                (actual - expected).abs() < 0.5,
                "at t={t}: camera x = {actual}, the same easing on a property gives {expected}"
            );
        }
    }

    #[test]
    fn a_parameterised_camera_easing_is_not_linear() {
        // Guards the actual regression: falling back to linear would still
        // hit both endpoints, so only the middle of the curve reveals it.
        let params = serde_json::json!([0.9, 0.0, 0.9, 1.0]);
        let scene = scene_with_camera("cubic_bezier", Some(params));
        let mid = Timeline::from_scene(&scene).get_camera_at(0.5, &scene).x;
        assert!(
            (mid - 50.0).abs() > 5.0,
            "a strongly-eased curve should not pass through the linear midpoint; got {mid}"
        );
    }

    #[test]
    fn plain_easings_still_work_without_parameters() {
        let scene = scene_with_camera("ease_out_cubic", None);
        let timeline = Timeline::from_scene(&scene);
        assert!((timeline.get_camera_at(0.0, &scene).x).abs() < 1e-3);
        assert!((timeline.get_camera_at(1.0, &scene).x - 100.0).abs() < 1e-3);
        // ease_out starts fast, so the midpoint is past halfway.
        assert!(timeline.get_camera_at(0.5, &scene).x > 55.0);
    }
}

/// Camera rotation, added in `AAA-MOT-05`.
///
/// The field is `#[serde(default)]`, which makes the interesting cases the
/// ones about *absence*: a scene written before the field existed must render
/// exactly as it did, and a rotation the author wrote must be interpolated as
/// the angle they wrote rather than the shortest way round.
#[cfg(test)]
mod camera_rotation {
    use crate::Timeline;
    use luminafx_schema::{Camera, CameraState, CameraTimelineEntry, Scene};

    fn bare_scene() -> Scene {
        serde_json::from_value(serde_json::json!({
            "version": "1.0",
            "meta": { "title": "t", "author": "a", "created_at": "2026-01-01T00:00:00Z" },
            "canvas": { "width": 64, "height": 64, "fps": 30, "duration": 2.0,
                        "background": "#000000" },
            "objects": {},
            "timeline": []
        }))
        .expect("fixture")
    }

    fn scene_rotating(from: f32, to: f32) -> Scene {
        let mut scene = bare_scene();
        scene.camera = Some(Camera {
            timeline: vec![
                CameraTimelineEntry {
                    time: 0.0,
                    state: CameraState {
                        x: 0.0,
                        y: 0.0,
                        zoom: 1.0,
                        rotation: from,
                    },
                    easing: "linear".into(),
                    easing_params: None,
                },
                CameraTimelineEntry {
                    time: 1.0,
                    state: CameraState {
                        x: 0.0,
                        y: 0.0,
                        zoom: 1.0,
                        rotation: to,
                    },
                    easing: "linear".into(),
                    easing_params: None,
                },
            ],
        });
        scene
    }

    #[test]
    fn a_camera_without_the_field_still_parses() {
        // Every camera in every scene authored before this change omits
        // `rotation`. If that stopped deserialising, the field would be a
        // breaking change to LSF rather than an addition to it.
        let json = r#"{ "timeline": [
            { "time": 0.0, "state": { "x": 1, "y": 2, "zoom": 1.5 }, "easing": "linear" }
        ] }"#;
        let camera: Camera = serde_json::from_str(json).expect("legacy camera must parse");
        assert_eq!(camera.timeline[0].state.rotation, 0.0);
    }

    #[test]
    fn rotation_interpolates_with_the_other_components() {
        let scene = scene_rotating(0.0, 90.0);
        let timeline = Timeline::from_scene(&scene);
        assert_eq!(timeline.get_camera_at(0.0, &scene).rotation, 0.0);
        assert_eq!(timeline.get_camera_at(0.5, &scene).rotation, 45.0);
        assert_eq!(timeline.get_camera_at(1.0, &scene).rotation, 90.0);
    }

    #[test]
    fn a_full_turn_is_a_full_turn() {
        // Shortest-arc interpolation would make this camera turn 10 degrees
        // backwards instead of 350 forwards — reversing the author's stated
        // direction, and making a full revolution unexpressible at all.
        let scene = scene_rotating(0.0, 350.0);
        let timeline = Timeline::from_scene(&scene);
        let mid = timeline.get_camera_at(0.5, &scene).rotation;
        assert_eq!(mid, 175.0, "the camera took the short way round");
    }

    #[test]
    fn a_scene_with_no_camera_is_unrotated() {
        let scene = bare_scene();
        let rotation = Timeline::from_scene(&scene)
            .get_camera_at(0.5, &scene)
            .rotation;
        assert_eq!(rotation, 0.0);
    }
}
