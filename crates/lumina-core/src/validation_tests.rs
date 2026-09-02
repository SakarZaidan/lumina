#[cfg(test)]
mod tests {
    use crate::validation::validate_scene_data;
    use lumina_schema::{Camera, CameraState, CameraTimelineEntry, Scene, TimelineEntry};

    fn scene_with_easing(easing: &str) -> Scene {
        let json = serde_json::json!({
            "version": "1.0",
            "meta": { "title": "t", "author": "a", "created_at": "2026-01-01T00:00:00Z" },
            "canvas": { "width": 100, "height": 100, "fps": 30, "duration": 2.0, "background": "#000000" },
            "objects": {
                "dot": { "type": "Circle", "properties": { "cx": 10, "cy": 10, "radius": 5 } }
            },
            "timeline": []
        });
        let mut scene: Scene = serde_json::from_value(json).unwrap();
        scene.timeline.push(TimelineEntry {
            time: 1.0,
            object: "dot".to_string(),
            state: serde_json::json!({ "cx": 50 }),
            easing: easing.to_string(),
            easing_params: None,
        });
        scene
    }

    #[test]
    fn unknown_easing_is_rejected_with_suggestion() {
        let result = validate_scene_data(&scene_with_easing("ease_in_ou"));
        assert!(!result.valid);
        let err = result
            .errors
            .iter()
            .find(|e| e.code == "UNKNOWN_EASING")
            .expect("UNKNOWN_EASING error expected");
        assert_eq!(err.path, "$.timeline[0].easing");
        assert!(
            err.fix_suggestion.contains("ease_in_out"),
            "suggestion should name the nearest easing, got: {}",
            err.fix_suggestion
        );
    }

    #[test]
    fn known_easing_passes() {
        let result = validate_scene_data(&scene_with_easing("ease_out_bounce"));
        assert!(result.valid, "errors: {:?}", result.errors);
    }

    #[test]
    fn parameterized_easing_without_params_warns() {
        let result = validate_scene_data(&scene_with_easing("cubic_bezier"));
        assert!(result.valid, "missing params is a warning, not an error");
        assert!(result
            .warnings
            .iter()
            .any(|w| w.code == "MISSING_EASING_PARAMS"));
    }

    #[test]
    fn camera_easing_is_validated() {
        let mut scene = scene_with_easing("linear");
        scene.camera = Some(Camera {
            timeline: vec![CameraTimelineEntry {
                time: 0.0,
                state: CameraState {
                    x: 0.0,
                    y: 0.0,
                    zoom: 1.0,
                },
                easing: "zoom_zoom".to_string(),
            }],
        });
        let result = validate_scene_data(&scene);
        assert!(!result.valid);
        assert!(result
            .errors
            .iter()
            .any(|e| e.code == "UNKNOWN_EASING" && e.path == "$.camera.timeline[0].easing"));
    }
}

/// Adversarial inputs: every case here is a small scene that asks for an
/// unbounded amount of work, and every one was accepted before the resource
/// bounds landed. Each asserts a specific error code so a future refactor
/// that removes a bound fails loudly rather than quietly.
#[cfg(test)]
mod resource_bounds {
    use crate::validation::validate_scene_data;
    use lumina_schema::Scene;

    /// Build a scene from JSON, merging `canvas` and `objects` overrides.
    ///
    /// Takes owned `Value`s so callers can pass `json!(...)` literals directly;
    /// borrowing would put a `&` in front of every fixture for no gain.
    #[allow(clippy::needless_pass_by_value)]
    fn scene(canvas: serde_json::Value, objects: serde_json::Value) -> Scene {
        let json = serde_json::json!({
            "version": "1.0",
            "meta": { "title": "t", "author": "a", "created_at": "2026-01-01T00:00:00Z" },
            "canvas": canvas,
            "objects": objects,
            "timeline": []
        });
        serde_json::from_value(json).expect("fixture must deserialise")
    }

    fn default_canvas() -> serde_json::Value {
        serde_json::json!({
            "width": 100, "height": 100, "fps": 30,
            "duration": 1.0, "background": "#000000"
        })
    }

    fn codes(scene: &Scene) -> Vec<String> {
        validate_scene_data(scene)
            .errors
            .into_iter()
            .map(|e| e.code)
            .collect()
    }

    fn assert_rejected(scene: &Scene, code: &str) {
        let found = codes(scene);
        assert!(
            found.iter().any(|c| c == code),
            "expected {code}, got {found:?}"
        );
        assert!(!validate_scene_data(scene).valid, "scene must not be valid");
    }

    #[test]
    fn a_canvas_larger_than_the_gpu_texture_limit_is_rejected() {
        // 65535 x 65535 RGBA is ~17 GB, allocated once per frame.
        let s = scene(
            serde_json::json!({
                "width": 65535, "height": 65535, "fps": 30,
                "duration": 1.0, "background": "#000000"
            }),
            serde_json::json!({}),
        );
        assert_rejected(&s, "CANVAS_TOO_LARGE");
    }

    #[test]
    fn an_enormous_frame_count_is_rejected() {
        // 30 bytes of JSON asking for 2.4e11 frames.
        let s = scene(
            serde_json::json!({
                "width": 100, "height": 100, "fps": 240,
                "duration": 1e9, "background": "#000000"
            }),
            serde_json::json!({}),
        );
        // duration alone is over the cap, and so is the product.
        let found = codes(&s);
        assert!(
            found
                .iter()
                .any(|c| c == "DURATION_TOO_LONG" || c == "TOO_MANY_FRAMES"),
            "expected a duration or frame-count error, got {found:?}"
        );
    }

    #[test]
    fn duration_and_fps_may_each_be_reasonable_while_their_product_is_not() {
        // Neither factor trips its own limit; the render still would.
        let s = scene(
            serde_json::json!({
                "width": 100, "height": 100, "fps": 240,
                "duration": 80000.0, "background": "#000000"
            }),
            serde_json::json!({}),
        );
        assert_rejected(&s, "TOO_MANY_FRAMES");
    }

    #[test]
    fn a_zero_frame_rate_is_rejected() {
        let s = scene(
            serde_json::json!({
                "width": 100, "height": 100, "fps": 0,
                "duration": 1.0, "background": "#000000"
            }),
            serde_json::json!({}),
        );
        assert_rejected(&s, "INVALID_FPS");
    }

    #[test]
    fn an_unbounded_plot_sample_count_is_rejected() {
        let s = scene(
            default_canvas(),
            serde_json::json!({
                "ax": { "type": "Axes", "properties": {
                    "x_range": [0.0, 10.0], "y_range": [0.0, 10.0], "x": 0.0, "y": 0.0
                }},
                "p": { "type": "Plot", "properties": {
                    "function_str": "sin(x)", "axes_id": "ax", "sample_count": 4000000000u32
                }}
            }),
        );
        assert_rejected(&s, "SAMPLE_COUNT_TOO_HIGH");
    }

    #[test]
    fn an_unbounded_expression_is_rejected() {
        let s = scene(
            default_canvas(),
            serde_json::json!({
                "ax": { "type": "Axes", "properties": {
                    "x_range": [0.0, 10.0], "y_range": [0.0, 10.0], "x": 0.0, "y": 0.0
                }},
                "p": { "type": "Plot", "properties": {
                    "function_str": "(".repeat(50_000), "axes_id": "ax"
                }}
            }),
        );
        assert_rejected(&s, "EXPRESSION_TOO_LONG");
    }

    #[test]
    fn an_unbounded_particle_count_is_rejected() {
        let s = scene(
            default_canvas(),
            serde_json::json!({
                "burst": { "type": "Particles", "properties": {
                    "count": 4000000000u32, "emitter_x": 0.0, "emitter_y": 0.0
                }}
            }),
        );
        assert_rejected(&s, "PARTICLE_COUNT_TOO_HIGH");
    }

    #[test]
    fn a_zero_axis_step_is_rejected_rather_than_saturating_a_cast() {
        // ((max - min) / 0.0).ceil() is inf; `inf as i32` saturates to
        // i32::MAX, so the tick loop ran 2.1 billion times per frame.
        let s = scene(
            default_canvas(),
            serde_json::json!({
                "ax": { "type": "Axes", "properties": {
                    "x_range": [0.0, 10.0], "y_range": [0.0, 10.0],
                    "x": 0.0, "y": 0.0, "x_step": 0.0
                }}
            }),
        );
        assert_rejected(&s, "INVALID_STEP");
    }

    #[test]
    fn a_negative_axis_step_is_rejected() {
        let s = scene(
            default_canvas(),
            serde_json::json!({
                "ax": { "type": "Axes", "properties": {
                    "x_range": [0.0, 10.0], "y_range": [0.0, 10.0],
                    "x": 0.0, "y": 0.0, "y_step": -1.0
                }}
            }),
        );
        assert_rejected(&s, "INVALID_STEP");
    }

    #[test]
    fn a_number_line_with_1e15_ticks_is_rejected() {
        let s = scene(
            default_canvas(),
            serde_json::json!({
                "nl": { "type": "NumberLine", "properties": {
                    "start": 0.0, "end": 1e9, "step": 1e-6, "x": 0.0, "y": 0.0
                }}
            }),
        );
        assert_rejected(&s, "TOO_MANY_TICKS");
    }

    #[test]
    fn deep_group_nesting_is_rejected_instead_of_overflowing_the_stack() {
        // A straight chain contains no cycle, so the `visited` set never
        // trips and depth is the only thing standing between this and a
        // stack overflow — during *validation*, before any render limit.
        let depth = 5_000;
        let mut objects = serde_json::Map::new();
        for i in 0..depth {
            let child = if i + 1 < depth {
                vec![format!("g{}", i + 1)]
            } else {
                vec![]
            };
            objects.insert(
                format!("g{i}"),
                serde_json::json!({
                    "type": "Group",
                    "properties": { "x": 0.0, "y": 0.0, "children": child }
                }),
            );
        }
        let s = scene(default_canvas(), serde_json::Value::Object(objects));
        assert_rejected(&s, "GROUP_NESTING_TOO_DEEP");
    }

    #[test]
    fn an_ordinary_scene_still_validates() {
        // The bounds must not reject anything anyone would actually write.
        let s = scene(
            serde_json::json!({
                "width": 1920, "height": 1080, "fps": 60,
                "duration": 120.0, "background": "#0F0F1A"
            }),
            serde_json::json!({
                "ax": { "type": "Axes", "properties": {
                    "x_range": [-10.0, 10.0], "y_range": [-5.0, 5.0],
                    "x": 100.0, "y": 400.0, "x_step": 1.0, "y_step": 1.0
                }},
                "curve": { "type": "Plot", "properties": {
                    "function_str": "sin(x) * cos(x / 2)", "axes_id": "ax", "sample_count": 500
                }},
                "sparks": { "type": "Particles", "properties": {
                    "count": 2000, "emitter_x": 640.0, "emitter_y": 360.0
                }},
                "line": { "type": "NumberLine", "properties": {
                    "start": -100.0, "end": 100.0, "step": 0.5, "x": 0.0, "y": 0.0
                }}
            }),
        );
        let response = validate_scene_data(&s);
        assert!(
            response.valid,
            "a normal scene must still validate; got {:?}",
            response.errors
        );
    }
}

/// The easing solvers assume preconditions the parameter *shape* checks do not
/// cover. Violating one used to produce a silently wrong curve rather than an
/// error — the worst outcome for a declarative format, because there is nothing
/// for an author or a self-correcting loop to act on.
#[cfg(test)]
mod easing_preconditions {
    use crate::validation::validate_scene_data;
    use lumina_schema::{Scene, TimelineEntry};

    fn scene_with_params(easing: &str, params: serde_json::Value) -> Scene {
        let json = serde_json::json!({
            "version": "1.0",
            "meta": { "title": "t", "author": "a", "created_at": "2026-01-01T00:00:00Z" },
            "canvas": { "width": 100, "height": 100, "fps": 30, "duration": 2.0, "background": "#000000" },
            "objects": {
                "dot": { "type": "Circle", "properties": { "cx": 10, "cy": 10, "radius": 5 } }
            },
            "timeline": []
        });
        let mut scene: Scene = serde_json::from_value(json).expect("fixture");
        scene.timeline.push(TimelineEntry {
            time: 1.0,
            object: "dot".to_string(),
            state: serde_json::json!({ "cx": 50 }),
            easing: easing.to_string(),
            easing_params: Some(params),
        });
        scene
    }

    fn codes(scene: &Scene) -> Vec<String> {
        validate_scene_data(scene)
            .errors
            .into_iter()
            .map(|e| e.code)
            .collect()
    }

    #[test]
    fn cubic_bezier_x_control_points_outside_the_unit_interval_are_rejected() {
        // Newton and bisection both need bezier_x monotonic in t, which the
        // CSS spec guarantees by constraining x1 and x2 to [0, 1].
        for params in [
            serde_json::json!([1.5, 0.0, 0.5, 1.0]),
            serde_json::json!([0.5, 0.0, -0.2, 1.0]),
        ] {
            let s = scene_with_params("cubic_bezier", params.clone());
            assert!(
                codes(&s).iter().any(|c| c == "INVALID_CUBIC_BEZIER"),
                "expected rejection for {params}, got {:?}",
                codes(&s)
            );
        }
    }

    #[test]
    fn cubic_bezier_y_control_points_may_leave_the_unit_interval() {
        // Overshoot is a legitimate effect and is expressed exactly this way.
        let s = scene_with_params("cubic_bezier", serde_json::json!([0.5, -0.8, 0.5, 1.8]));
        assert!(
            validate_scene_data(&s).valid,
            "y control points outside [0,1] are how overshoot is written: {:?}",
            validate_scene_data(&s).errors
        );
    }

    #[test]
    fn unsorted_spline_keypoints_are_rejected() {
        // Unsorted input clamps a negative interval to 1e-9, so the tangent
        // becomes ~1e9 and the output is garbage that then reads as `null`.
        let s = scene_with_params(
            "spline",
            serde_json::json!({ "keypoints": [[0.0, 0.0], [0.8, 0.5], [0.3, 1.0]] }),
        );
        assert!(
            codes(&s).iter().any(|c| c == "UNSORTED_SPLINE_KEYPOINTS"),
            "got {:?}",
            codes(&s)
        );
    }

    #[test]
    fn duplicate_spline_keypoint_times_are_rejected() {
        let s = scene_with_params(
            "spline",
            serde_json::json!({ "keypoints": [[0.0, 0.0], [0.5, 0.4], [0.5, 1.0]] }),
        );
        assert!(
            codes(&s).iter().any(|c| c == "UNSORTED_SPLINE_KEYPOINTS"),
            "got {:?}",
            codes(&s)
        );
    }

    #[test]
    fn well_formed_easing_params_still_validate() {
        let bezier = scene_with_params("cubic_bezier", serde_json::json!([0.25, 0.1, 0.25, 1.0]));
        assert!(validate_scene_data(&bezier).valid);

        let spline = scene_with_params(
            "spline",
            serde_json::json!({ "keypoints": [[0.0, 0.0], [0.5, 0.8], [1.0, 1.0]] }),
        );
        assert!(validate_scene_data(&spline).valid);

        let spring = scene_with_params(
            "spring",
            serde_json::json!({ "stiffness": 300.0, "damping": 25.0, "mass": 1.0 }),
        );
        assert!(validate_scene_data(&spring).valid);
    }
}

/// The engine renders in `f32`. A number that does not survive that conversion
/// does not fail loudly — `serde_json` encodes the resulting infinity as
/// `null`, the property vanishes from the state map, and the renderer
/// substitutes its own default. The animation is wrong and nothing says so.
#[cfg(test)]
mod representable_numbers {
    use crate::validation::validate_scene_data;
    use lumina_schema::{Scene, TimelineEntry};

    fn scene_with_state(state: serde_json::Value) -> Scene {
        let json = serde_json::json!({
            "version": "1.0",
            "meta": { "title": "t", "author": "a", "created_at": "2026-01-01T00:00:00Z" },
            "canvas": { "width": 100, "height": 100, "fps": 30, "duration": 2.0, "background": "#000000" },
            "objects": {
                "dot": { "type": "Circle", "properties": { "cx": 10, "cy": 10, "radius": 5 } }
            },
            "timeline": []
        });
        let mut scene: Scene = serde_json::from_value(json).expect("fixture");
        scene.timeline.push(TimelineEntry {
            time: 1.0,
            object: "dot".to_string(),
            state,
            easing: "linear".to_string(),
            easing_params: None,
        });
        scene
    }

    fn codes(scene: &Scene) -> Vec<String> {
        validate_scene_data(scene)
            .errors
            .into_iter()
            .map(|e| e.code)
            .collect()
    }

    #[test]
    fn a_keyframe_value_that_overflows_f32_is_rejected() {
        // 1e39 parses as f64 without complaint and becomes inf as f32.
        let s = scene_with_state(serde_json::json!({ "cx": 1e39 }));
        assert!(
            codes(&s).iter().any(|c| c == "NUMBER_NOT_REPRESENTABLE"),
            "got {:?}",
            codes(&s)
        );
    }

    #[test]
    fn overflow_inside_an_array_is_rejected() {
        // Point lists and gradient stops are arrays, so the check recurses.
        let s = scene_with_state(serde_json::json!({ "points": [[0.0, 0.0], [1e40, 3.0]] }));
        assert!(
            codes(&s).iter().any(|c| c == "NUMBER_NOT_REPRESENTABLE"),
            "got {:?}",
            codes(&s)
        );
    }

    #[test]
    fn ordinary_magnitudes_are_accepted() {
        // Including values that are large but perfectly representable.
        let s = scene_with_state(serde_json::json!({
            "cx": 1920.0, "radius": 1e30, "opacity": 0.5, "points": [[-1e20, 1e20]]
        }));
        assert!(
            validate_scene_data(&s).valid,
            "got {:?}",
            validate_scene_data(&s).errors
        );
    }
}

/// Colour strings the renderer cannot parse must be reported, not drawn white.
#[cfg(test)]
mod colours {
    use crate::validation::validate_scene_data;
    use lumina_schema::Scene;

    fn scene_with_fill(fill: &str) -> Scene {
        serde_json::from_value(serde_json::json!({
            "version": "1.0",
            "meta": { "title": "t", "author": "a", "created_at": "2026-01-01T00:00:00Z" },
            "canvas": { "width": 64, "height": 64, "fps": 30, "duration": 1.0,
                        "background": "#000000" },
            "objects": {
                "c": { "type": "Circle", "properties": {
                    "cx": 10.0, "cy": 10.0, "radius": 5.0, "fill": fill } }
            },
            "timeline": []
        }))
        .expect("fixture")
    }

    fn codes(scene: &Scene) -> Vec<String> {
        validate_scene_data(scene)
            .errors
            .into_iter()
            .map(|e| e.code)
            .collect()
    }

    #[test]
    fn valid_hex_forms_are_accepted() {
        for fill in ["#FFF", "#ffffff", "#FF00FF80", "#abc"] {
            assert!(
                validate_scene_data(&scene_with_fill(fill)).valid,
                "{fill} should be accepted"
            );
        }
    }

    #[test]
    fn unparseable_colours_are_rejected() {
        // Each of these rendered as opaque white with no diagnostic.
        for fill in ["red", "#GGGGGG", "#12345", "rgb(1,2,3)", ""] {
            assert!(
                codes(&scene_with_fill(fill))
                    .iter()
                    .any(|c| c == "INVALID_COLOR"),
                "{fill:?} should be rejected, got {:?}",
                codes(&scene_with_fill(fill))
            );
        }
    }

    #[test]
    fn none_gets_a_specific_suggestion() {
        // `fill="none"` is an SVG habit and the most likely mistake, so it is
        // worth telling the author what to write instead.
        let scene = scene_with_fill("none");
        let errors = validate_scene_data(&scene).errors;
        let e = errors
            .iter()
            .find(|e| e.code == "INVALID_COLOR")
            .expect("rejected");
        assert!(
            e.fix_suggestion.contains("Omit") || e.fix_suggestion.contains("alpha"),
            "should suggest the alternative: {}",
            e.fix_suggestion
        );
    }

    #[test]
    fn an_invalid_background_is_rejected() {
        let mut scene = scene_with_fill("#FFFFFF");
        scene.canvas.background = "transparent".into();
        assert!(codes(&scene).iter().any(|c| c == "INVALID_COLOR"));
    }
}
