#[cfg(test)]
mod tests {
    use crate::validation::validate_scene_data;
    use luminafx_schema::{Camera, CameraState, CameraTimelineEntry, Scene, TimelineEntry};

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
                    rotation: 0.0,
                },
                easing: "zoom_zoom".to_string(),
                easing_params: None,
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
    use luminafx_schema::Scene;

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
    use luminafx_schema::{Scene, TimelineEntry};

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
    use luminafx_schema::{Scene, TimelineEntry};

    /// A circle `dot` and a polygon `shape`, with one keyframe per entry in
    /// `states`, each an `(object, state)` pair.
    fn scene_with_states(states: &[(&str, serde_json::Value)]) -> Scene {
        let json = serde_json::json!({
            "version": "1.0",
            "meta": { "title": "t", "author": "a", "created_at": "2026-01-01T00:00:00Z" },
            "canvas": { "width": 100, "height": 100, "fps": 30, "duration": 2.0, "background": "#000000" },
            "objects": {
                "dot": { "type": "Circle", "properties": { "cx": 10, "cy": 10, "radius": 5 } },
                "shape": { "type": "Polygon",
                           "properties": { "points": [[0, 0], [10, 0], [0, 10]] } }
            },
            "timeline": []
        });
        let mut scene: Scene = serde_json::from_value(json).expect("fixture");
        for (object, state) in states {
            scene.timeline.push(TimelineEntry {
                time: 1.0,
                object: (*object).to_string(),
                state: state.clone(),
                easing: "linear".to_string(),
                easing_params: None,
            });
        }
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
        let s = scene_with_states(&[("dot", serde_json::json!({ "cx": 1e39 }))]);
        assert!(
            codes(&s).iter().any(|c| c == "NUMBER_NOT_REPRESENTABLE"),
            "got {:?}",
            codes(&s)
        );
    }

    #[test]
    fn overflow_inside_an_array_is_rejected() {
        // Point lists and gradient stops are arrays, so the check recurses.
        let s = scene_with_states(&[(
            "shape",
            serde_json::json!({ "points": [[0.0, 0.0], [1e40, 3.0]] }),
        )]);
        assert!(
            codes(&s).iter().any(|c| c == "NUMBER_NOT_REPRESENTABLE"),
            "got {:?}",
            codes(&s)
        );
    }

    #[test]
    fn ordinary_magnitudes_are_accepted() {
        // Including values that are large but perfectly representable.
        let s = scene_with_states(&[
            (
                "dot",
                serde_json::json!({ "cx": 1920.0, "radius": 1e30, "opacity": 0.5 }),
            ),
            ("shape", serde_json::json!({ "points": [[-1e20, 1e20]] })),
        ]);
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
    use luminafx_schema::Scene;

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

/// Unknown-identifier diagnostics (`AAA-AI-08`).
///
/// Two of these references were not validated at all, and both failed in the
/// quietest way available: the object simply did not appear, and the render
/// succeeded. Every other dangling reference in a scene is an error; these two
/// were silence.
#[cfg(test)]
mod unknown_references {
    use crate::validation::validate_scene_data;
    use luminafx_schema::Scene;

    fn scene(objects: &serde_json::Value, assets: &serde_json::Value) -> Scene {
        serde_json::from_value(serde_json::json!({
            "version": "1.0",
            "meta": { "title": "t", "author": "a", "created_at": "2026-01-01T00:00:00Z" },
            "canvas": { "width": 64, "height": 64, "fps": 30, "duration": 1.0,
                        "background": "#000000" },
            "assets": assets,
            "objects": objects,
            "timeline": []
        }))
        .expect("scene")
    }

    fn find<'a>(
        r: &'a crate::validation::ValidationResponse,
        code: &str,
    ) -> Option<&'a crate::validation::ValidationError> {
        r.errors.iter().find(|e| e.code == code)
    }

    #[test]
    fn a_typoed_asset_id_is_an_error_rather_than_an_invisible_object() {
        let s = scene(
            &serde_json::json!({
                "logo": { "type": "Image",
                          "properties": { "asset_id": "brnd", "x": 0, "y": 0 } }
            }),
            &serde_json::json!({ "images": [{ "id": "brand", "path": "brand.png" }] }),
        );
        let r = validate_scene_data(&s);
        let e = find(&r, "UNKNOWN_ASSET_ID").expect("must be reported");
        assert!(
            e.fix_suggestion.contains("brand"),
            "no suggestion for a one-character typo: {}",
            e.fix_suggestion
        );
    }

    #[test]
    fn a_correct_asset_reference_passes() {
        let s = scene(
            &serde_json::json!({
                "logo": { "type": "Image",
                          "properties": { "asset_id": "brand", "x": 0, "y": 0 } }
            }),
            &serde_json::json!({ "images": [{ "id": "brand", "path": "brand.png" }] }),
        );
        assert!(find(&validate_scene_data(&s), "UNKNOWN_ASSET_ID").is_none());
    }

    #[test]
    fn a_plot_pointing_at_nothing_is_an_error() {
        let s = scene(
            &serde_json::json!({
                "ax": { "type": "Axes",
                        "properties": { "x_range": [0, 10], "y_range": [0, 10],
                                        "x": 0, "y": 0 } },
                "p": { "type": "Plot",
                       "properties": { "function_str": "x", "axes_id": "axes" } }
            }),
            &serde_json::json!({}),
        );
        let r = validate_scene_data(&s);
        let e = find(&r, "UNKNOWN_AXES_ID").expect("must be reported");
        assert!(e.fix_suggestion.contains("ax"), "{}", e.fix_suggestion);
    }

    #[test]
    fn a_plot_pointing_at_the_wrong_kind_of_object_says_which_kind() {
        // Distinct from "not declared": the id resolves, so telling the author
        // to add an object would send them to fix the wrong thing.
        let s = scene(
            &serde_json::json!({
                "c": { "type": "Circle",
                       "properties": { "cx": 1, "cy": 1, "radius": 1 } },
                "p": { "type": "Plot",
                       "properties": { "function_str": "x", "axes_id": "c" } }
            }),
            &serde_json::json!({}),
        );
        let r = validate_scene_data(&s);
        let e = find(&r, "AXES_ID_IS_NOT_AXES").expect("must be reported");
        assert!(e.message.contains("Circle"), "{}", e.message);
    }

    #[test]
    fn a_transposed_object_id_is_now_suggested() {
        // The old prefix match compared the first three characters, so a
        // transposition in that range produced no suggestion at all.
        let mut s = scene(
            &serde_json::json!({
                "circle_one": { "type": "Circle",
                                "properties": { "cx": 1, "cy": 1, "radius": 1 } }
            }),
            &serde_json::json!({}),
        );
        s.timeline = serde_json::from_value(serde_json::json!([
            { "time": 0.0, "object": "cricle_one", "state": { "radius": 2.0 } }
        ]))
        .expect("timeline");
        let r = validate_scene_data(&s);
        let e = find(&r, "UNKNOWN_OBJECT_ID").expect("must be reported");
        assert!(
            e.fix_suggestion.contains("circle_one"),
            "a transposition went unsuggested: {}",
            e.fix_suggestion
        );
    }

    #[test]
    fn an_unrelated_id_is_not_confidently_mismatched() {
        // The other half of the old behaviour: a shared three-character prefix
        // was enough to propose a completely different object.
        let mut s = scene(
            &serde_json::json!({
                "titan_orbit_diagram": { "type": "Circle",
                                         "properties": { "cx": 1, "cy": 1, "radius": 1 } }
            }),
            &serde_json::json!({}),
        );
        s.timeline = serde_json::from_value(serde_json::json!([
            { "time": 0.0, "object": "title", "state": { "radius": 2.0 } }
        ]))
        .expect("timeline");
        let r = validate_scene_data(&s);
        let e = find(&r, "UNKNOWN_OBJECT_ID").expect("must be reported");
        assert!(
            !e.fix_suggestion.contains("titan_orbit_diagram"),
            "an unrelated object was suggested: {}",
            e.fix_suggestion
        );
    }
}

/// Property names and types, checked against the structs (RFC-0002 Stage 1).
///
/// The first four tests are the reproductions from the RFC, run against `main`
/// before any of this was written: each passed validation and rendered wrong.
#[cfg(test)]
mod typed_properties {
    use crate::validation::validate_scene_json;
    use serde_json::{json, Value};

    fn scene(properties: &Value, timeline: &Value) -> Value {
        let mut props = json!({ "cx": 32, "cy": 32, "radius": 10, "fill": "#FF0000" });
        if let (Some(base), Some(extra)) = (props.as_object_mut(), properties.as_object()) {
            for (k, v) in extra {
                base.insert(k.clone(), v.clone());
            }
        }
        json!({
            "version": "1.0",
            "meta": { "title": "t", "author": "a", "created_at": "2026-01-01T00:00:00Z" },
            "canvas": { "width": 64, "height": 64, "fps": 30, "duration": 2.0,
                        "background": "#000000" },
            "objects": { "c": { "type": "Circle", "properties": props } },
            "timeline": timeline
        })
    }

    fn codes(v: &Value) -> Vec<String> {
        validate_scene_json(v)
            .errors
            .iter()
            .map(|e| e.code.clone())
            .collect()
    }

    #[test]
    fn a_correct_scene_has_no_property_errors() {
        let r = validate_scene_json(&scene(
            &json!({}),
            &json!([{ "time": 1.0, "object": "c", "state": { "radius": 20 } }]),
        ));
        assert!(r.valid, "a correct scene was rejected: {:?}", r.errors);
    }

    #[test]
    fn a_misspelled_animated_property_is_an_error_with_a_suggestion() {
        // Repro 1: the animation silently did nothing.
        let r = validate_scene_json(&scene(
            &json!({}),
            &json!([{ "time": 1.0, "object": "c", "state": { "raduis": 20 } }]),
        ));
        let e = r
            .errors
            .iter()
            .find(|e| e.code == "UNKNOWN_PROPERTY")
            .expect("must be reported");
        assert_eq!(e.path, "$.timeline[0].state.raduis");
        assert!(e.fix_suggestion.contains("radius"), "{}", e.fix_suggestion);
    }

    #[test]
    fn a_wrong_typed_animated_property_is_an_error_naming_the_type() {
        // Repro 2: read back as a missing number, so the circle vanished.
        let r = validate_scene_json(&scene(
            &json!({}),
            &json!([{ "time": 1.0, "object": "c", "state": { "radius": "big" } }]),
        ));
        let e = r
            .errors
            .iter()
            .find(|e| e.code == "PROPERTY_TYPE_MISMATCH")
            .expect("must be reported");
        assert_eq!(e.path, "$.timeline[0].state.radius");
        assert!(e.message.contains("a number"), "{}", e.message);
        assert!(e.message.contains("a string"), "{}", e.message);
    }

    #[test]
    fn an_unknown_static_property_is_an_error_even_though_serde_drops_it() {
        // Repro 3: serde discarded the field before any check could see it —
        // which is why this has to read the raw document.
        let r = validate_scene_json(&scene(&json!({ "opacty": 0.5 }), &json!([])));
        let e = r
            .errors
            .iter()
            .find(|e| e.code == "UNKNOWN_PROPERTY")
            .expect("must be reported");
        assert_eq!(e.path, "$.objects.c.properties.opacty");
        assert!(e.fix_suggestion.contains("opacity"), "{}", e.fix_suggestion);
    }

    #[test]
    fn a_wrong_type_in_properties_is_reported_with_a_path_not_a_serde_message() {
        // serde rejects this too, but with no path and no suggestion. The
        // property error must be what comes back, not PARSE_ERROR.
        let c = codes(&scene(&json!({ "radius": "big" }), &json!([])));
        assert!(c.contains(&"PROPERTY_TYPE_MISMATCH".to_string()), "{c:?}");
        assert!(!c.contains(&"PARSE_ERROR".to_string()), "{c:?}");
    }

    #[test]
    fn a_misspelled_object_type_suggests_the_real_one() {
        let mut s = scene(&json!({}), &json!([]));
        s["objects"]["c"]["type"] = json!("Cirle");
        let r = validate_scene_json(&s);
        let e = r
            .errors
            .iter()
            .find(|e| e.code == "UNKNOWN_OBJECT_TYPE")
            .expect("must be reported");
        assert!(e.fix_suggestion.contains("Circle"), "{}", e.fix_suggestion);
    }

    #[test]
    fn an_entry_for_a_missing_object_reports_only_the_missing_object() {
        // Not one UNKNOWN_OBJECT_ID plus a pile of property errors derived from
        // it — that buries the one real mistake.
        let c = codes(&scene(
            &json!({}),
            &json!([{ "time": 1.0, "object": "nope", "state": { "anything": 1 } }]),
        ));
        assert!(c.contains(&"UNKNOWN_OBJECT_ID".to_string()), "{c:?}");
        assert!(!c.contains(&"UNKNOWN_PROPERTY".to_string()), "{c:?}");
    }

    #[test]
    fn integer_properties_accept_the_fractions_interpolation_produces() {
        // A timeline interpolates z_index through fractional values. Rejecting
        // 2.5 would reject a scene the engine renders correctly.
        let r = validate_scene_json(&scene(
            &json!({ "z_index": 1 }),
            &json!([{ "time": 1.0, "object": "c", "state": { "z_index": 2.5 } }]),
        ));
        assert!(r.valid, "{:?}", r.errors);
    }

    #[test]
    fn optional_properties_accept_null() {
        let r = validate_scene_json(&scene(
            &json!({ "stroke": null, "shadow": null }),
            &json!([]),
        ));
        assert!(r.valid, "{:?}", r.errors);
    }

    #[test]
    fn every_object_type_has_a_derived_property_list() {
        // If a variant stopped resolving, every property on it would become
        // UNKNOWN_PROPERTY. This catches that before any scene does.
        let schema = crate::property_schema::PropertySchema::get();
        let mut types: Vec<&str> = schema.object_types().collect();
        types.sort_unstable();
        assert_eq!(types.len(), 17, "object types derived: {types:?}");
        for t in &types {
            let props = schema.properties_of(t).expect("listed");
            assert!(!props.is_empty(), "{t} derived no properties");
        }
    }

    #[test]
    fn integer_properties_are_known_to_be_integers() {
        // The timeline rounds these when it animates them; a property wrongly
        // classed as an integer would have its animation snapped to whole
        // numbers, and one wrongly classed as fractional would freeze its
        // object mid-transition.
        let schema = crate::property_schema::PropertySchema::get();
        let kinds = |ty: &str, prop: &str| schema.properties_of(ty).expect(ty)[prop];
        for (ty, prop) in [
            ("Circle", "z_index"),
            ("Plot", "sample_count"),
            ("Particles", "count"),
        ] {
            assert!(kinds(ty, prop).is_integer(), "{ty}.{prop}");
        }
        for (ty, prop) in [
            ("Circle", "radius"),
            ("Circle", "fill"),
            ("Rectangle", "width"),
        ] {
            assert!(!kinds(ty, prop).is_integer(), "{ty}.{prop}");
        }
        assert!(
            kinds("Circle", "z_index").accepts(&serde_json::json!(2.5)),
            "an integer property must still accept a fraction on validation"
        );
    }

    /// The RFC-0002 gate: every scene this repository ships still validates.
    ///
    /// This is the test that decides whether Stage 1 is safe to ship. The
    /// adapter reads property types out of a generated schema, and a single
    /// mis-resolved `$ref` or composition keyword would make it reject a scene
    /// the engine renders perfectly well — for every user of that scene.
    #[test]
    fn every_shipped_scene_still_validates() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("workspace root");
        let mut checked = 0;
        let mut failures = Vec::new();
        for dir in ["examples", "crates/lumina-renderer/tests/fixtures"] {
            let Ok(entries) = std::fs::read_dir(root.join(dir)) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("lsf") {
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(&path) else {
                    continue;
                };
                let Ok(raw) = serde_json::from_str::<Value>(&text) else {
                    continue;
                };
                checked += 1;
                let r = validate_scene_json(&raw);
                let property_errors: Vec<_> = r
                    .errors
                    .iter()
                    .filter(|e| {
                        matches!(
                            e.code.as_str(),
                            "UNKNOWN_PROPERTY"
                                | "PROPERTY_TYPE_MISMATCH"
                                | "UNKNOWN_OBJECT_TYPE"
                                | "PROPERTY_VALUE_INVALID"
                                | "TIMELINE_STATE_NOT_AN_OBJECT"
                                | "UNKNOWN_OBJECT_ID"
                        )
                    })
                    .map(|e| format!("{} {} — {}", e.code, e.path, e.message))
                    .collect();
                if !property_errors.is_empty() {
                    failures.push(format!(
                        "{}:\n    {}",
                        path.display(),
                        property_errors.join("\n    ")
                    ));
                }
            }
        }
        assert!(
            checked > 10,
            "only {checked} scenes found — the walk is broken"
        );
        assert!(
            failures.is_empty(),
            "{} of {checked} shipped scenes are now rejected:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }
}

/// `font_id` on `LaTeX` and `MathML` reaches the renderer's state.
///
/// Both backends read `state["font_id"]` for `Text`, `LaTeX` and `MathML` in one
/// shared text branch, but only `TextProps` declared the field. Serde dropped it
/// from the other two on parse and `Timeline::from_scene` re-serialised the
/// struct without it, so the renderer's lookup could only ever find nothing:
/// `showcase_grand` and `showcase_neural_network` asked their formulas for the
/// bold font and got the regular one. Found by RFC-0002's property validation
/// rejecting those examples on its first run.
#[cfg(test)]
mod font_id_reaches_state {
    use crate::Timeline;
    use luminafx_schema::Scene;
    use serde_json::json;

    fn state_font(object_type: &str, content_key: &str) -> Option<String> {
        let scene: Scene = serde_json::from_value(json!({
            "version": "1.0",
            "meta": { "title": "t", "author": "a", "created_at": "2026-01-01T00:00:00Z" },
            "canvas": { "width": 64, "height": 64, "fps": 30, "duration": 1.0,
                        "background": "#000000" },
            "objects": { "f": { "type": object_type, "properties": {
                content_key: "x", "x": 0, "y": 0, "font_size": 20, "font_id": "bold"
            } } },
            "timeline": []
        }))
        .expect("scene");
        Timeline::from_scene(&scene).get_state_at(0.0)["f"]["font_id"]
            .as_str()
            .map(String::from)
    }

    #[test]
    fn latex_keeps_its_font() {
        assert_eq!(state_font("LaTeX", "expression").as_deref(), Some("bold"));
    }

    #[test]
    fn mathml_keeps_its_font() {
        assert_eq!(state_font("MathML", "markup").as_deref(), Some("bold"));
    }

    #[test]
    fn text_still_keeps_its_font() {
        // The path that always worked, as the control.
        assert_eq!(state_font("Text", "content").as_deref(), Some("bold"));
    }
}

/// What timeline entries and event actions assign: checked by name, kind and
/// shape, from every entry point.
#[cfg(test)]
mod assignments {
    use crate::validation::{validate_scene_data, validate_scene_json, ValidationError};
    use luminafx_schema::Scene;
    use serde_json::{json, Value};

    fn document(timeline: &Value, events: &Value) -> Value {
        let mut scene = json!({
            "version": "1.0",
            "meta": { "title": "t", "author": "a", "created_at": "2026-01-01T00:00:00Z" },
            "canvas": { "width": 64, "height": 64, "fps": 30, "duration": 2.0,
                        "background": "#000000" },
            "objects": {
                "a": { "type": "Arrow", "properties": { "from": [8, 8], "to": [56, 56] } },
                "c": { "type": "Circle", "properties": { "cx": 32, "cy": 32, "radius": 10 } }
            }
        });
        scene["timeline"] = timeline.clone();
        scene["events"] = events.clone();
        scene
    }

    /// Errors from the typed entry point, which every caller holding a parsed
    /// `Scene` uses — the server's `/render` among them.
    fn typed(timeline: &Value, events: &Value) -> Vec<ValidationError> {
        let scene: Scene = serde_json::from_value(document(timeline, events)).expect("scene");
        validate_scene_data(&scene).errors
    }

    fn the_one<'e>(errors: &'e [ValidationError], code: &str) -> &'e ValidationError {
        let matching: Vec<_> = errors.iter().filter(|e| e.code == code).collect();
        assert_eq!(matching.len(), 1, "expected one {code}, got {errors:?}");
        matching[0]
    }

    #[test]
    fn a_keyframe_of_the_right_kind_and_the_wrong_shape_is_an_error() {
        // An array where an array belongs, one element short of a point. The
        // engine cannot build the Arrow, so it used to draw it as authored and
        // ignore the keyframe without a word.
        let errors = typed(
            &json!([{ "time": 1.0, "object": "a", "state": { "from": [8.0] } }]),
            &json!([]),
        );
        let e = the_one(&errors, "PROPERTY_VALUE_INVALID");
        assert_eq!(e.path, "$.timeline[0].state.from");
        assert!(e.message.contains("length"), "{}", e.message);
        assert!(
            e.fix_suggestion.contains("[8.0,8.0]"),
            "the suggestion should show the object's own value: {}",
            e.fix_suggestion
        );
    }

    #[test]
    fn a_fraction_for_an_integer_property_is_not_an_error() {
        // The timeline rounds it, so the value is fine as written.
        let errors = typed(
            &json!([{ "time": 1.0, "object": "c", "state": { "z_index": 2.5 } }]),
            &json!([]),
        );
        assert!(errors.is_empty(), "{errors:?}");
    }

    #[test]
    fn a_state_that_is_not_an_object_is_an_error() {
        let errors = typed(
            &json!([{ "time": 1.0, "object": "c", "state": [1, 2] }]),
            &json!([]),
        );
        assert_eq!(
            the_one(&errors, "TIMELINE_STATE_NOT_AN_OBJECT").path,
            "$.timeline[0].state"
        );
    }

    #[test]
    fn the_typed_entry_point_catches_a_misspelled_keyframe() {
        // Before, only `validate_scene_json` looked at timeline names, so a
        // caller holding a parsed scene accepted what `/validate` rejected.
        let errors = typed(
            &json!([{ "time": 1.0, "object": "c", "state": { "raduis": 20 } }]),
            &json!([]),
        );
        assert!(the_one(&errors, "UNKNOWN_PROPERTY")
            .fix_suggestion
            .contains("radius"));
    }

    #[test]
    fn a_misspelled_keyframe_is_reported_once_from_raw_json() {
        let raw = document(
            &json!([{ "time": 1.0, "object": "c", "state": { "raduis": 20 } }]),
            &json!([]),
        );
        the_one(&validate_scene_json(&raw).errors, "UNKNOWN_PROPERTY");
    }

    #[test]
    fn a_keyframe_problem_is_still_reported_when_the_scene_does_not_parse() {
        // `radius` as a string stops the objects parsing, so there is no typed
        // scene to check the timeline with. The raw pass still reports the
        // keyframe, so one round reports both.
        let mut raw = document(
            &json!([{ "time": 1.0, "object": "c", "state": { "raduis": 20 } }]),
            &json!([]),
        );
        raw["objects"]["c"]["properties"]["radius"] = json!("big");
        let errors = validate_scene_json(&raw).errors;
        the_one(&errors, "PROPERTY_TYPE_MISMATCH");
        the_one(&errors, "UNKNOWN_PROPERTY");
    }

    fn set_property(target: &str, property: &str, value: &Value) -> Value {
        json!([{ "object": "c", "trigger": "click",
                 "action": { "type": "set_property", "target": target,
                             "property": property, "value": value } }])
    }

    #[test]
    fn an_action_targeting_a_missing_object_is_an_error() {
        let errors = typed(&json!([]), &set_property("circel", "radius", &json!(5)));
        let e = the_one(&errors, "UNKNOWN_OBJECT_ID");
        assert_eq!(e.path, "$.events[0].action.target");
    }

    #[test]
    fn an_action_setting_a_misspelled_property_is_an_error() {
        let errors = typed(&json!([]), &set_property("c", "opacty", &json!(0.5)));
        let e = the_one(&errors, "UNKNOWN_PROPERTY");
        assert_eq!(e.path, "$.events[0].action.property");
        assert!(e.fix_suggestion.contains("opacity"), "{}", e.fix_suggestion);
    }

    #[test]
    fn an_action_value_of_the_wrong_kind_or_shape_is_an_error() {
        let errors = typed(&json!([]), &set_property("c", "radius", &json!("big")));
        assert_eq!(
            the_one(&errors, "PROPERTY_TYPE_MISMATCH").path,
            "$.events[0].action.value"
        );

        let errors = typed(&json!([]), &set_property("a", "to", &json!([1, 2, 3])));
        assert_eq!(
            the_one(&errors, "PROPERTY_VALUE_INVALID").path,
            "$.events[0].action.value"
        );
    }

    #[test]
    fn a_placeholder_value_is_only_judged_by_its_property_name() {
        // `$drag.to` becomes the host's payload when the event fires, so a
        // string standing in for an array is not a mistake.
        let errors = typed(&json!([]), &set_property("a", "to", &json!("$drag.to")));
        assert!(errors.is_empty(), "{errors:?}");

        let errors = typed(&json!([]), &set_property("a", "too", &json!("$drag.to")));
        the_one(&errors, "UNKNOWN_PROPERTY");
    }
}

/// Paints that cannot be painted: an unrecognised colour, or a gradient the
/// renderer would drop. Both used to come out opaque white with nothing said.
#[cfg(test)]
mod paints {
    use crate::validation::{validate_scene_json, ValidationResponse};
    use serde_json::{json, Value};

    fn scene(fill: Value, timeline: Value) -> ValidationResponse {
        let mut document = json!({
            "version": "1.0",
            "meta": { "title": "t", "author": "a", "created_at": "2026-01-01T00:00:00Z" },
            "canvas": { "width": 64, "height": 64, "fps": 30, "duration": 1.0,
                        "background": "#000000" },
            "objects": { "c": { "type": "Circle",
                                "properties": { "cx": 1, "cy": 1, "radius": 5 } } }
        });
        document["objects"]["c"]["properties"]["fill"] = fill;
        document["timeline"] = timeline;
        validate_scene_json(&document)
    }

    fn gradient(kind: &str, stops: Value) -> Value {
        let mut gradient = json!({ "type": kind });
        gradient["stops"] = stops;
        gradient
    }

    fn codes(response: &ValidationResponse) -> Vec<&str> {
        response.errors.iter().map(|e| e.code.as_str()).collect()
    }

    #[test]
    fn a_gradient_needs_two_stops() {
        let one = scene(gradient("linear", json!([[0.0, "#FF0000"]])), json!([]));
        assert_eq!(codes(&one), ["GRADIENT_TOO_FEW_STOPS"]);
        let two = scene(
            gradient("linear", json!([[0.0, "#FF0000"], [1.0, "#0000FF"]])),
            json!([]),
        );
        assert!(two.valid, "{:?}", two.errors);
    }

    #[test]
    fn a_stop_whose_colour_is_not_a_colour_is_named_precisely() {
        let response = scene(
            gradient("radial", json!([[0.0, "#FF0000"], [1.0, "nope"]])),
            json!([]),
        );
        let error = response
            .errors
            .iter()
            .find(|e| e.code == "INVALID_COLOR")
            .expect("reported");
        assert_eq!(error.path, "$.objects.c.properties.fill.stops[1][1]");
    }

    #[test]
    fn an_unknown_gradient_type_is_a_warning_because_it_still_draws() {
        let response = scene(
            gradient("conic", json!([[0.0, "#FF0000"], [1.0, "#0000FF"]])),
            json!([]),
        );
        assert!(response.valid, "{:?}", response.errors);
        assert_eq!(response.warnings[0].code, "UNKNOWN_GRADIENT_TYPE");
    }

    #[test]
    fn a_colour_a_keyframe_sets_is_checked_too() {
        // Authored colours were checked; animated ones were not, so a typo in
        // a keyframe turned the shape white halfway through.
        let response = scene(
            json!("#FF0000"),
            json!([{ "time": 1.0, "object": "c", "state": { "fill": "#GGGGGG" } }]),
        );
        let error = response
            .errors
            .iter()
            .find(|e| e.code == "INVALID_COLOR")
            .expect("reported");
        assert_eq!(error.path, "$.timeline[0].state.fill");
    }

    #[test]
    fn none_is_still_not_a_colour_wherever_it_appears() {
        let response = scene(
            json!("#FF0000"),
            json!([{ "time": 1.0, "object": "c", "state": { "fill": "none" } }]),
        );
        let error = response
            .errors
            .iter()
            .find(|e| e.code == "INVALID_COLOR")
            .expect("reported");
        assert!(
            error.fix_suggestion.contains("alpha"),
            "{}",
            error.fix_suggestion
        );
    }
}
