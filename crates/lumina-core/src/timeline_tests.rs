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
            fill: Some("#FFF".into()),
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

/// A shadow with no color is black (it was white until v0.6).
#[cfg(test)]
mod shadow_default {
    use crate::Timeline;
    use luminafx_schema::Scene;
    use serde_json::json;

    #[test]
    fn an_uncoloured_shadow_reaches_the_renderer_as_black() {
        // Asserted on the state the renderer reads, because that is where the
        // wrong default won: the schema's value is applied on parse and is always
        // present by then, so the renderer's own black fallback never ran.
        let scene: Scene = serde_json::from_value(json!({
            "version": "1.0",
            "meta": { "title": "t", "author": "a", "created_at": "2026-01-01T00:00:00Z" },
            "canvas": { "width": 64, "height": 64, "fps": 30, "duration": 1.0,
                        "background": "#FFFFFF" },
            "objects": { "c": { "type": "Circle", "properties": {
                "cx": 32, "cy": 32, "radius": 10, "shadow": { "blur": 4, "dx": 2, "dy": 2 } } } },
            "timeline": []
        }))
        .expect("scene");
        let state = Timeline::from_scene(&scene).get_state_at(0.0);
        assert_eq!(state["c"]["shadow"]["color"], "#000000");
    }
}

/// `Timeline::resolve_at` — typed objects at a time (RFC-0002 Stage 2).
#[cfg(test)]
mod resolve_at {
    use crate::Timeline;
    use luminafx_schema::{Object, Scene};
    use serde_json::json;

    fn scene() -> Scene {
        serde_json::from_value(json!({
            "version": "1.0",
            "meta": { "title": "t", "author": "a", "created_at": "2026-01-01T00:00:00Z" },
            "canvas": { "width": 64, "height": 64, "fps": 30, "duration": 2.0,
                        "background": "#000000" },
            "objects": {
                "c": { "type": "Circle",
                       "properties": { "cx": 32, "cy": 32, "radius": 10, "fill": "#FF0000" } },
                "still": { "type": "Rectangle",
                           "properties": { "x": 0, "y": 0, "width": 4, "height": 4 } }
            },
            "timeline": [
                { "time": 0.0, "object": "c", "state": { "radius": 10 } },
                { "time": 2.0, "object": "c", "state": { "radius": 20 } }
            ]
        }))
        .expect("scene")
    }

    #[test]
    fn an_animated_property_arrives_typed_and_interpolated() {
        let resolved = Timeline::from_scene(&scene()).resolve_at(1.0);
        let Some(Object::Circle(c)) = resolved.get("c") else {
            panic!("circle did not resolve to a Circle");
        };
        assert!(
            (c.radius - 15.0).abs() < 1e-4,
            "radius at t=1 is {}",
            c.radius
        );
    }

    #[test]
    fn every_object_resolves_including_static_ones() {
        let resolved = Timeline::from_scene(&scene()).resolve_at(1.0);
        assert_eq!(
            resolved.len(),
            2,
            "an object went missing: {:?}",
            resolved.keys()
        );
        assert!(matches!(resolved.get("still"), Some(Object::Rectangle(_))));
    }

    #[test]
    fn defaults_are_the_schemas_not_the_renderers() {
        // The renderer reads `stroke_width` with `unwrap_or(1.0)`; the schema
        // says 0.0. Typed state has exactly one answer, and it is the schema's.
        let resolved = Timeline::from_scene(&scene()).resolve_at(0.0);
        let Some(Object::Circle(c)) = resolved.get("c") else {
            panic!("not a circle");
        };
        assert_eq!(c.stroke_width, 0.0);
    }

    #[test]
    fn an_unresolvable_override_falls_back_to_the_authored_object() {
        // A wrong-typed interactive override on a scene nobody validated. The
        // object must come back as authored, never be dropped: dropping it
        // would make it vanish from the frame, which is the silent failure
        // RFC-0002 exists to remove.
        let mut timeline = Timeline::from_scene(&scene());
        timeline.override_property("c", "radius", json!("big"));
        let resolved = timeline.resolve_at(1.0);
        let Some(Object::Circle(c)) = resolved.get("c") else {
            panic!("the circle vanished instead of falling back");
        };
        assert_eq!(
            c.radius, 10.0,
            "fell back to something other than the authored value"
        );
    }

    #[test]
    fn an_override_replaces_its_track_instead_of_colliding_with_it() {
        // `radius` has a track and an override. Handed to serde as two
        // entries, the struct would name the field twice and fail to
        // deserialise, and the fallback would silently discard the override
        // together with every other animated value on the object.
        let mut timeline = Timeline::from_scene(&scene());
        timeline.override_property("c", "radius", json!(42));
        let resolved = timeline.resolve_at(1.0);
        let Some(Object::Circle(c)) = resolved.get("c") else {
            panic!("not a circle");
        };
        assert_eq!(c.radius, 42.0);
    }

    #[test]
    fn an_override_reaches_an_object_nothing_animates() {
        // `still` has no timeline entries, so it resolves without evaluating
        // anything. An override has to take it off that path.
        let mut timeline = Timeline::from_scene(&scene());
        timeline.override_property("still", "width", json!(9));
        let resolved = timeline.resolve_at(1.0);
        let Some(Object::Rectangle(r)) = resolved.get("still") else {
            panic!("not a rectangle");
        };
        assert_eq!(r.width, 9.0);
    }

    fn integer_scene() -> Scene {
        serde_json::from_value(json!({
            "version": "1.0",
            "meta": { "title": "t", "author": "a", "created_at": "2026-01-01T00:00:00Z" },
            "canvas": { "width": 64, "height": 64, "fps": 30, "duration": 2.0,
                        "background": "#000000" },
            "objects": {
                "c": { "type": "Circle",
                       "properties": { "cx": 0, "cy": 32, "radius": 10, "z_index": 0 } }
            },
            "timeline": [
                { "time": 0.0, "object": "c", "state": { "cx": 0, "z_index": 0 } },
                { "time": 2.0, "object": "c", "state": { "cx": 100, "z_index": 5 } }
            ]
        }))
        .expect("scene")
    }

    #[test]
    fn an_animated_integer_is_rounded_rather_than_freezing_its_object() {
        // Interpolation makes `z_index` 2.5 halfway, which is not an `i32`.
        // Unrounded, the circle fails to deserialise and falls back to its
        // authored self — so its `cx` animation stops dead for the whole
        // transition, though nothing is wrong with `cx`.
        let resolved = Timeline::from_scene(&integer_scene()).resolve_at(1.0);
        let Some(Object::Circle(c)) = resolved.get("c") else {
            panic!("not a circle");
        };
        assert!((c.cx - 50.0).abs() < 1e-3, "cx froze at {}", c.cx);
        // Halves round toward positive infinity, as CSS rounds an animated
        // integer.
        assert_eq!(c.z_index, 3);
    }

    #[test]
    fn the_untyped_state_carries_the_same_rounded_integer() {
        // The untyped path had its own version of the bug: both renderers read
        // a particle `count` with `as_u64`, which is `None` for any
        // interpolated number, whole or not — so particles vanished between
        // keyframes.
        let state = Timeline::from_scene(&integer_scene()).get_state_at(1.0);
        assert_eq!(state["c"]["z_index"], json!(3));
        assert!(state["c"]["z_index"].as_u64().is_some());
    }

    #[test]
    fn a_fractional_override_of_an_integer_is_rounded() {
        let mut timeline = Timeline::from_scene(&integer_scene());
        timeline.override_property("c", "z_index", json!(6.4));
        let resolved = timeline.resolve_at(1.0);
        let Some(Object::Circle(c)) = resolved.get("c") else {
            panic!("not a circle");
        };
        assert_eq!(c.z_index, 6);
        assert!((c.cx - 50.0).abs() < 1e-3, "cx froze at {}", c.cx);
    }

    /// The untyped state deserialised the obvious way: the reference
    /// `resolve_at` has to agree with.
    fn from_untyped_state(authored: &Object, state: Option<&serde_json::Value>) -> Object {
        let Some(props) = state else {
            return authored.clone();
        };
        let mut tagged = serde_json::to_value(authored).expect("objects serialise");
        tagged["properties"] = props.clone();
        serde_json::from_value(tagged).unwrap_or_else(|_| authored.clone())
    }

    #[test]
    fn resolving_agrees_with_the_untyped_state_on_every_shipped_scene() {
        // What lets the renderer move onto `resolve_at` without changing a
        // pixel. It takes two shortcuts past the untyped state — no
        // intermediate map, and no evaluation at all for an object nothing
        // animates — and both have to land exactly where the long way does.
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("workspace root");
        let mut checked = 0;
        for dir in ["examples", "crates/lumina-renderer/tests/fixtures"] {
            let Ok(entries) = std::fs::read_dir(root.join(dir)) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("lsf") {
                    continue;
                }
                let Some(scene) = std::fs::read_to_string(&path)
                    .ok()
                    .and_then(|text| serde_json::from_str::<Scene>(&text).ok())
                else {
                    continue;
                };
                checked += 1;
                let timeline = Timeline::from_scene(&scene);
                let d = scene.canvas.duration;
                for time in [0.0, d * 0.25, d * 0.5, d * 0.75, d, d + 1.0] {
                    let state = timeline.get_state_at(time);
                    let resolved = timeline.resolve_at(time);
                    assert_eq!(resolved.len(), scene.objects.len(), "{}", path.display());
                    for (id, authored) in &scene.objects {
                        let expected = from_untyped_state(authored, state.get(id));
                        assert_eq!(
                            serde_json::to_value(&resolved[id]).expect("serialise"),
                            serde_json::to_value(&expected).expect("serialise"),
                            "{} disagrees on {id} at t={time}",
                            path.display()
                        );
                    }
                }
            }
        }
        assert!(
            checked >= 20,
            "only {checked} scenes found; the walk is broken"
        );
    }
}

/// A quick ratio check for `resolve_at`, run by hand: `cargo test --release
/// -p luminafx-core resolve_cost -- --ignored --nocapture`.
///
/// Ignored because timing in a unit test is noise on a shared CI runner; the
/// criterion `resolve_eval` group is the measurement of record. This exists to
/// compare the two paths *in one process* while iterating on the
/// implementation — a ratio from one run survives a change of machine where an
/// absolute number does not.
#[cfg(test)]
mod resolve_cost {
    use crate::Timeline;
    use luminafx_schema::Scene;
    use serde_json::json;

    fn animated_scene(n: usize) -> Scene {
        let mut objects = serde_json::Map::new();
        let mut timeline = Vec::new();
        for i in 0..n {
            let id = format!("c{i}");
            objects.insert(
                id.clone(),
                json!({ "type": "Circle", "properties": {
                    "cx": (i % 100) as f64 * 10.0, "cy": (i / 100) as f64 * 10.0,
                    "radius": 5.0, "z_index": i, "fill": "#FF6B6B" } }),
            );
            timeline.push(
                json!({ "time": 0.0, "object": id, "state": { "cx": 0.0, "opacity": 0.0 },
                                  "easing": "ease_out_cubic" }),
            );
            timeline.push(
                json!({ "time": 2.0, "object": id, "state": { "cx": 200.0, "opacity": 1.0 },
                                  "easing": "ease_out_cubic" }),
            );
        }
        serde_json::from_value(json!({
            "version": "1.0",
            "meta": { "title": "t", "author": "a", "created_at": "2026-01-01T00:00:00Z" },
            "canvas": { "width": 1920, "height": 1080, "fps": 30, "duration": 4.0,
                        "background": "#000000" },
            "objects": objects, "timeline": timeline
        }))
        .expect("scene")
    }

    #[test]
    #[ignore = "timing; run by hand with --release"]
    fn resolve_cost_ratio() {
        for n in [100usize, 1000] {
            let timeline = Timeline::from_scene(&animated_scene(n));
            let iters = 20_000 / n.max(1) * 10;
            // Warm both paths so neither pays first-touch costs in the sample.
            for _ in 0..50 {
                std::hint::black_box(timeline.get_state_at(1.0));
                std::hint::black_box(timeline.resolve_at(1.0));
            }
            let t = std::time::Instant::now();
            for _ in 0..iters {
                std::hint::black_box(timeline.get_state_at(1.0));
            }
            let untyped = t.elapsed();
            let t = std::time::Instant::now();
            for _ in 0..iters {
                std::hint::black_box(timeline.resolve_at(1.0));
            }
            let typed = t.elapsed();
            println!(
                "n={n:5}  get_state_at {:>8.1?}/iter   resolve_at {:>8.1?}/iter   ratio {:.2}x",
                untyped / iters as u32,
                typed / iters as u32,
                typed.as_secs_f64() / untyped.as_secs_f64()
            );
        }
    }
}

/// Frame 0, where a keyframe at t = 0 and the authored value both apply.
#[cfg(test)]
mod first_frame {
    use crate::Timeline;
    use luminafx_schema::{Object, Scene};
    use serde_json::json;

    fn fade_in() -> Scene {
        serde_json::from_value(json!({
            "version": "1.0",
            "meta": { "title": "t", "author": "a", "created_at": "2026-01-01T00:00:00Z" },
            "canvas": { "width": 64, "height": 64, "fps": 30, "duration": 2.0,
                        "background": "#000000" },
            "objects": { "c": { "type": "Circle",
                "properties": { "cx": 32, "cy": 32, "radius": 10, "opacity": 1.0 } } },
            "timeline": [
                { "time": 0.0, "object": "c", "state": { "opacity": 0.0 } },
                { "time": 1.0, "object": "c", "state": { "opacity": 1.0 } }
            ]
        }))
        .expect("scene")
    }

    #[test]
    fn a_keyframe_at_zero_beats_the_authored_value_on_frame_zero() {
        // The authored `opacity: 1` sorts first, and frame 0 used to take it,
        // so this fade-in flashed fully opaque for one frame and then started
        // again from nearly transparent.
        let timeline = Timeline::from_scene(&fade_in());
        assert_eq!(timeline.get_state_at(0.0)["c"]["opacity"], json!(0.0));
        let Some(Object::Circle(c)) = timeline.resolve_at(0.0).remove("c") else {
            panic!("not a circle");
        };
        assert_eq!(c.opacity, 0.0);
    }

    #[test]
    fn frame_zero_leads_smoothly_into_frame_one() {
        let timeline = Timeline::from_scene(&fade_in());
        let at = |t: f32| {
            timeline.get_state_at(t)["c"]["opacity"]
                .as_f64()
                .unwrap_or(-1.0)
        };
        assert!(
            at(0.0) <= at(1.0 / 30.0),
            "opacity fell from frame 0 ({}) to frame 1 ({})",
            at(0.0),
            at(1.0 / 30.0)
        );
    }

    #[test]
    fn of_two_entries_at_zero_the_later_one_wins() {
        // The same rule as at any other time on a track, where the bracketing
        // search already lands on the last of a run of equal times.
        let mut scene = fade_in();
        scene.timeline.insert(
            1,
            serde_json::from_value(json!(
                { "time": 0.0, "object": "c", "state": { "opacity": 0.25 } }
            ))
            .expect("entry"),
        );
        let state = Timeline::from_scene(&scene).get_state_at(0.0);
        assert_eq!(state["c"]["opacity"], json!(0.25));
    }

    #[test]
    fn with_no_keyframe_at_zero_the_authored_value_holds() {
        let mut scene = fade_in();
        scene.timeline.remove(0);
        let state = Timeline::from_scene(&scene).get_state_at(0.0);
        assert_eq!(state["c"]["opacity"], json!(1.0));
    }
}
