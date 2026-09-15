//! Semantic scene validation.
//!
//! Lives in `lumina-core` so every consumer — server, CLI, SDKs — applies
//! the same rules; `lumina-server` re-exports these types unchanged.
//! Errors are render-blocking; warnings are advisory. `fix_suggestion` is
//! written for self-correction loops (both human and LLM authors).

use crate::easing::{is_valid_easing, suggest_easing};
use luminafx_schema::{Action, Object, Scene};
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

// ── Resource bounds ─────────────────────────────────────────────────────────
//
// A scene is a description of work, and until these existed nothing bounded
// how much work a small description could ask for. The server caps request
// bodies at 8 MiB, but `{"duration": 1e9, "fps": 240}` is 30 bytes and asks
// for 2.4e11 frames.
//
// Every limit is enforced here because `validate_scene_data` is the one
// chokepoint every entry point already calls — server, CLI, and both SDKs —
// so a bound added here cannot be bypassed by reaching the renderer another
// way. The numbers are chosen to sit far above any legitimate scene and far
// below anything that threatens the host.

/// Largest canvas dimension, in pixels. 16384 is the common GPU texture
/// limit, and a 16384² RGBA frame is already 1 GiB.
pub const MAX_CANVAS_DIMENSION: u32 = 16_384;
/// Largest frame rate. Beyond this the output is not a video anyone watches.
pub const MAX_FPS: u32 = 240;
/// Longest scene, in seconds — a little over 24 hours.
pub const MAX_DURATION_SECONDS: f32 = 86_400.0;
/// Largest total frame count for one render (`duration × fps`).
pub const MAX_TOTAL_FRAMES: u64 = 1_000_000;
/// Largest `sample_count` on a `Plot`. Sampling is per frame.
pub const MAX_PLOT_SAMPLES: u32 = 100_000;
/// Largest `count` on a `Particles` emitter. Simulation is per frame.
pub const MAX_PARTICLE_COUNT: u32 = 1_000_000;
/// Largest number of tick marks a `NumberLine` or `Axes` may derive from its
/// range and step. Each tick is a stroked path, drawn every frame.
pub const MAX_TICK_COUNT: f64 = 100_000.0;
/// Largest `function_str` on a `Plot`, in bytes. evalexpr parses by recursive
/// descent, so an unbounded expression is an unbounded stack.
pub const MAX_EXPRESSION_BYTES: usize = 4_096;
/// Most temporal supersamples per frame. Cost is linear in this, on top of
/// the frame count — 64 samples of a 1 000 000-frame render is 64 million
/// renders.
pub const MAX_MOTION_BLUR_SAMPLES: u32 = 64;

/// Deepest chain of nested groups. Bounds the recursive walks in cycle
/// detection here and in scene traversal in the renderers.
pub const MAX_GROUP_DEPTH: usize = 256;

#[derive(Debug, Serialize, Clone)]
/// Outcome of [`validate_scene_data`].
pub struct ValidationResponse {
    /// True when there are no render-blocking errors.
    pub valid: bool,
    /// Render-blocking problems.
    pub errors: Vec<ValidationError>,
    /// Advisory findings that never block rendering.
    pub warnings: Vec<ValidationWarning>,
}

#[derive(Debug, Serialize, Clone)]
/// A render-blocking validation finding.
pub struct ValidationError {
    /// Stable machine-readable code (e.g. `UNKNOWN_EASING`).
    pub code: String,
    /// JSONPath-style location of the offending value.
    pub path: String,
    /// Human-readable description.
    pub message: String,
    /// Actionable correction, written for self-fixing authoring loops.
    pub fix_suggestion: String,
}

#[derive(Debug, Serialize, Clone)]
/// An advisory validation finding.
pub struct ValidationWarning {
    /// Stable machine-readable code (e.g. `DUPLICATE_KEYFRAME`).
    pub code: String,
    /// JSONPath-style location of the value concerned.
    pub path: String,
    /// Human-readable description.
    pub message: String,
}

/// Perform semantic validation of a parsed Scene.
/// Returns errors (render-blocking) and warnings (non-blocking).
pub fn validate_scene_data(scene: &Scene) -> ValidationResponse {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    let object_ids: HashSet<&str> = scene.objects.keys().map(|s| s.as_str()).collect();

    // Check 1: Timeline entries must reference declared object IDs
    for (i, entry) in scene.timeline.iter().enumerate() {
        if !object_ids.contains(entry.object.as_str()) {
            let suggestion = crate::suggest::did_you_mean(
                &entry.object,
                object_ids.iter().copied(),
                "Check the 'objects' block for valid IDs.",
            );

            errors.push(ValidationError {
                code: "UNKNOWN_OBJECT_ID".to_string(),
                path: format!("$.timeline[{i}].object"),
                message: format!(
                    "Timeline entry {} references object '{}', which is not in 'objects'.",
                    i, entry.object
                ),
                fix_suggestion: suggestion,
            });
        }
    }

    // Check 2: Event entries must reference declared object IDs
    for (i, event) in scene.events.iter().enumerate() {
        if !object_ids.contains(event.object.as_str()) {
            errors.push(ValidationError {
                code: "UNKNOWN_OBJECT_ID".to_string(),
                path: format!("$.events[{i}].object"),
                message: format!(
                    "Event {} references object '{}', which is not declared.",
                    i, event.object
                ),
                fix_suggestion: crate::suggest::did_you_mean(
                    &event.object,
                    object_ids.iter().copied(),
                    &format!(
                        "Add '{}' to the 'objects' block or correct the event's object field.",
                        event.object
                    ),
                ),
            });
        }
    }

    // Check 2b: Image/SVG objects must reference a declared asset.
    //
    // This was not checked at all, and the failure mode is the quietest kind:
    // a typo'd `asset_id` draws nothing, so the object simply is not there and
    // the render succeeds. Every other unknown reference in a scene is an
    // error, and this one was silence.
    {
        let asset_ids: std::collections::HashSet<&str> =
            scene.assets.images.iter().map(|a| a.id.as_str()).collect();
        for (obj_id, obj) in &scene.objects {
            let asset_id = match obj {
                Object::Image(p) => Some(&p.asset_id),
                Object::SVG(p) => Some(&p.asset_id),
                _ => None,
            };
            if let Some(asset_id) = asset_id {
                if !asset_ids.contains(asset_id.as_str()) {
                    errors.push(ValidationError {
                        code: "UNKNOWN_ASSET_ID".to_string(),
                        path: format!("$.objects.{obj_id}.properties.asset_id"),
                        message: format!(
                            "Object '{obj_id}' references asset '{asset_id}', which is not \
                             declared in 'assets.images'."
                        ),
                        fix_suggestion: crate::suggest::did_you_mean(
                            asset_id,
                            asset_ids.iter().copied(),
                            "Declare it under 'assets.images' with an id and a path.",
                        ),
                    });
                }
            }
        }
    }

    // Check 2c: a Plot must reference an Axes that exists, and is an Axes.
    //
    // Also unchecked. A plot with a dangling `axes_id` has no coordinate
    // system to draw in, so it too rendered as nothing at all.
    for (obj_id, obj) in &scene.objects {
        if let Object::Plot(p) = obj {
            match scene.objects.get(&p.axes_id) {
                Some(Object::Axes(_)) => {}
                Some(other) => errors.push(ValidationError {
                    code: "AXES_ID_IS_NOT_AXES".to_string(),
                    path: format!("$.objects.{obj_id}.properties.axes_id"),
                    message: format!(
                        "Plot '{obj_id}' references '{}', which is a {} rather than an Axes.",
                        p.axes_id,
                        object_type_name(other)
                    ),
                    fix_suggestion: "A Plot draws inside an Axes; point axes_id at one."
                        .to_string(),
                }),
                None => errors.push(ValidationError {
                    code: "UNKNOWN_AXES_ID".to_string(),
                    path: format!("$.objects.{obj_id}.properties.axes_id"),
                    message: format!(
                        "Plot '{obj_id}' references axes '{}', which is not declared.",
                        p.axes_id
                    ),
                    fix_suggestion: crate::suggest::did_you_mean(
                        &p.axes_id,
                        scene
                            .objects
                            .iter()
                            .filter(|(_, o)| matches!(o, Object::Axes(_)))
                            .map(|(k, _)| k.as_str()),
                        "Add an Axes object and point axes_id at it.",
                    ),
                }),
            }
        }
    }

    // Check 3: Group children must reference declared object IDs
    for (obj_id, obj) in &scene.objects {
        if let Object::Group(group) = obj {
            for child_id in &group.children {
                if !object_ids.contains(child_id.as_str()) {
                    errors.push(ValidationError {
                        code: "UNKNOWN_CHILD_ID".to_string(),
                        path: format!("$.objects.{obj_id}.properties.children"),
                        message: format!(
                            "Group '{obj_id}' references child '{child_id}', which is not declared."
                        ),
                        fix_suggestion: crate::suggest::did_you_mean(
                            child_id,
                            object_ids.iter().copied(),
                            &format!(
                                "Add '{child_id}' to the 'objects' block or remove it from \
                                 group '{obj_id}'."
                            ),
                        ),
                    });
                }
            }
        }
    }

    // Check 4: Circular group references, and nesting too deep to walk.
    // Both are found by the same traversal: a cycle trips the path check, a
    // straight chain trips the depth check. The depth case matters because
    // this walk runs *before* any render limit could apply, and overflowing
    // its stack aborts the process rather than failing the request.
    match detect_group_cycle(&scene.objects) {
        Some(GroupWalk::Cycle(cycle)) => errors.push(ValidationError {
            code: "CIRCULAR_GROUP_REFERENCE".to_string(),
            path: "$.objects".to_string(),
            message: format!(
                "Circular group reference: {}. Groups cannot contain themselves.",
                cycle.join(" → ")
            ),
            fix_suggestion: "Remove the circular dependency from the group's children list."
                .to_string(),
        }),
        Some(GroupWalk::TooDeep(id)) => errors.push(ValidationError {
            code: "GROUP_NESTING_TOO_DEEP".to_string(),
            path: format!("$.objects.{id}"),
            message: format!(
                "Group nesting exceeds {MAX_GROUP_DEPTH} levels at '{id}'. Scene traversal is \
                 recursive, so deeper nesting cannot be walked safely."
            ),
            fix_suggestion: "Flatten the group hierarchy. Nesting this deep is almost always a \
                             generated-scene bug rather than an authoring choice."
                .to_string(),
        }),
        None => {}
    }

    // Check 5: Keyframes beyond canvas duration (warning)
    for (i, entry) in scene.timeline.iter().enumerate() {
        if entry.time > scene.canvas.duration {
            warnings.push(ValidationWarning {
                code: "KEYFRAME_BEYOND_DURATION".to_string(),
                path: format!("$.timeline[{i}].time"),
                message: format!(
                    "Keyframe {} has time={:.2}s but canvas duration is {:.2}s. It will never play.",
                    i, entry.time, scene.canvas.duration
                ),
            });
        }
    }

    // Check 6: Duplicate keyframes (same object + property + time) — warning
    let mut seen: HashMap<(String, String, String), usize> = HashMap::new();
    for (i, entry) in scene.timeline.iter().enumerate() {
        if let serde_json::Value::Object(state) = &entry.state {
            for prop_name in state.keys() {
                let key = (
                    entry.object.clone(),
                    prop_name.clone(),
                    format!("{:.6}", entry.time),
                );
                if let Some(first_idx) = seen.get(&key) {
                    warnings.push(ValidationWarning {
                        code: "DUPLICATE_KEYFRAME".to_string(),
                        path: format!("$.timeline[{i}]"),
                        message: format!(
                            "Duplicate keyframe for '{}' property '{}' at t={:.2}s (first at index {}). Last declaration wins.",
                            entry.object, prop_name, entry.time, first_idx
                        ),
                    });
                } else {
                    seen.insert(key, i);
                }
            }
        }
    }

    // Check 7: Canvas dimensions must be positive
    if scene.canvas.width == 0 || scene.canvas.height == 0 {
        errors.push(ValidationError {
            code: "INVALID_CANVAS_SIZE".to_string(),
            path: "$.canvas".to_string(),
            message: format!(
                "Canvas size {}x{} is invalid. Both dimensions must be > 0.",
                scene.canvas.width, scene.canvas.height
            ),
            fix_suggestion:
                "Set canvas.width and canvas.height to positive integers (e.g. 1280, 720)."
                    .to_string(),
        });
    }

    // Check 7b: Canvas and timing must stay within renderable bounds.
    // Unbounded values here multiply into per-frame work — see the module
    // constants for why each limit is where it is.
    if scene.canvas.width > MAX_CANVAS_DIMENSION || scene.canvas.height > MAX_CANVAS_DIMENSION {
        errors.push(ValidationError {
            code: "CANVAS_TOO_LARGE".to_string(),
            path: "$.canvas".to_string(),
            message: format!(
                "Canvas size {}x{} exceeds the maximum dimension of {MAX_CANVAS_DIMENSION} px. \
                 A frame this size is allocated once per frame rendered.",
                scene.canvas.width, scene.canvas.height
            ),
            fix_suggestion: format!(
                "Reduce canvas.width and canvas.height to at most {MAX_CANVAS_DIMENSION}."
            ),
        });
    }

    if scene.canvas.fps == 0 {
        errors.push(ValidationError {
            code: "INVALID_FPS".to_string(),
            path: "$.canvas.fps".to_string(),
            message: "Canvas fps is 0, so the scene has no frames.".to_string(),
            fix_suggestion: "Set canvas.fps to a positive integer (e.g. 30 or 60).".to_string(),
        });
    } else if scene.canvas.fps > MAX_FPS {
        errors.push(ValidationError {
            code: "FPS_TOO_HIGH".to_string(),
            path: "$.canvas.fps".to_string(),
            message: format!(
                "Canvas fps {} exceeds the maximum of {MAX_FPS}.",
                scene.canvas.fps
            ),
            fix_suggestion: format!("Set canvas.fps to at most {MAX_FPS} (60 is typical)."),
        });
    }

    if !scene.canvas.duration.is_finite() || scene.canvas.duration <= 0.0 {
        errors.push(ValidationError {
            code: "INVALID_DURATION".to_string(),
            path: "$.canvas.duration".to_string(),
            message: format!(
                "Canvas duration {} is not a positive, finite number of seconds.",
                scene.canvas.duration
            ),
            fix_suggestion: "Set canvas.duration to a positive number of seconds.".to_string(),
        });
    } else if scene.canvas.duration > MAX_DURATION_SECONDS {
        errors.push(ValidationError {
            code: "DURATION_TOO_LONG".to_string(),
            path: "$.canvas.duration".to_string(),
            message: format!(
                "Canvas duration {}s exceeds the maximum of {MAX_DURATION_SECONDS}s.",
                scene.canvas.duration
            ),
            fix_suggestion: format!(
                "Set canvas.duration to at most {MAX_DURATION_SECONDS} seconds, or render the \
                 scene in sections."
            ),
        });
    }

    // The product is what actually bounds the render, and either factor can be
    // individually reasonable while the product is not.
    if scene.canvas.duration.is_finite() && scene.canvas.duration > 0.0 && scene.canvas.fps > 0 {
        let frames = (scene.canvas.duration as f64) * f64::from(scene.canvas.fps);
        if frames > MAX_TOTAL_FRAMES as f64 {
            errors.push(ValidationError {
                code: "TOO_MANY_FRAMES".to_string(),
                path: "$.canvas".to_string(),
                message: format!(
                    "duration {}s x fps {} is {frames:.0} frames, over the maximum of \
                     {MAX_TOTAL_FRAMES}.",
                    scene.canvas.duration, scene.canvas.fps
                ),
                fix_suggestion: format!(
                    "Reduce canvas.duration or canvas.fps so their product is at most \
                     {MAX_TOTAL_FRAMES} frames."
                ),
            });
        }
    }

    // Check 7c2: Motion blur multiplies the render cost by its sample count,
    // on top of every other per-frame bound.
    if scene.canvas.motion_blur_samples == 0 {
        errors.push(ValidationError {
            code: "INVALID_MOTION_BLUR".to_string(),
            path: "$.canvas.motion_blur_samples".to_string(),
            message: "motion_blur_samples is 0, so no frame would be rendered at all.".to_string(),
            fix_suggestion: "Use 1 for no motion blur, or 2-64 to enable it.".to_string(),
        });
    } else if scene.canvas.motion_blur_samples > MAX_MOTION_BLUR_SAMPLES {
        errors.push(ValidationError {
            code: "INVALID_MOTION_BLUR".to_string(),
            path: "$.canvas.motion_blur_samples".to_string(),
            message: format!(
                "motion_blur_samples {} exceeds the maximum of {MAX_MOTION_BLUR_SAMPLES}. \
                 Every sample is a full render of the frame.",
                scene.canvas.motion_blur_samples
            ),
            fix_suggestion: format!(
                "Use at most {MAX_MOTION_BLUR_SAMPLES}; 4 to 8 is enough for smooth blur."
            ),
        });
    }
    if scene.canvas.motion_blur_samples > 1
        && (!scene.canvas.shutter.is_finite()
            || scene.canvas.shutter <= 0.0
            || scene.canvas.shutter > 1.0)
    {
        errors.push(ValidationError {
            code: "INVALID_SHUTTER".to_string(),
            path: "$.canvas.shutter".to_string(),
            message: format!(
                "shutter {} must be within (0, 1] — the fraction of each frame interval the \
                 shutter is open.",
                scene.canvas.shutter
            ),
            fix_suggestion: "Use 0.5 for a 180-degree shutter, the film convention.".to_string(),
        });
    }

    // Check 7d: Keyframe values must survive the f32 conversion the engine
    // performs on every number. `1e39` parses happily as f64, becomes `inf`
    // as f32, and `serde_json` then encodes that as `null` — so the property
    // silently disappears from the state map and the renderer substitutes its
    // own default. Telling the author is the whole point of a validator.
    for (i, entry) in scene.timeline.iter().enumerate() {
        if let Value::Object(state) = &entry.state {
            for (prop, value) in state {
                check_representable(
                    value,
                    &format!("$.timeline[{i}].state.{prop}"),
                    &mut errors,
                    0,
                );
            }
        }
    }

    // Check 7e: Colours must parse. An unrecognised colour string silently
    // becomes opaque white in the renderer, so a typo — or an SVG habit like
    // `"none"` — renders as a white shape with nothing to say why.
    check_colour(&scene.canvas.background, "$.canvas.background", &mut errors);
    for (id, obj) in &scene.objects {
        if let Ok(serde_json::Value::Object(props)) =
            serde_json::to_value(obj).map(|v| v["properties"].clone())
        {
            for (name, value) in &props {
                if matches!(name.as_str(), "fill" | "stroke" | "color") {
                    // Gradients are objects, not strings; only literals are
                    // checked here.
                    if let Some(text) = value.as_str() {
                        check_colour(
                            text,
                            &format!("$.objects.{id}.properties.{name}"),
                            &mut errors,
                        );
                    }
                }
            }
        }
    }

    // Check 7c: Per-object work that is repeated every frame.
    for (id, obj) in &scene.objects {
        validate_object_bounds(id, obj, &mut errors);
    }

    // Check 8: Large scene warning
    if scene.objects.len() > 500 {
        warnings.push(ValidationWarning {
            code: "LARGE_SCENE".to_string(),
            path: "$.objects".to_string(),
            message: format!(
                "Scene has {} objects. Consider grouping related objects to improve performance.",
                scene.objects.len()
            ),
        });
    }

    // Check 9: Easing names must be recognized (timeline, events, camera).
    // Unknown names are errors — silent linear fallback hid typos (TD-08).
    for (i, entry) in scene.timeline.iter().enumerate() {
        check_easing(
            &entry.easing,
            entry.easing_params.as_ref(),
            format!("$.timeline[{i}].easing"),
            &mut errors,
            &mut warnings,
        );
    }
    for (i, event) in scene.events.iter().enumerate() {
        if let Action::TweenTo { easing, .. } = &event.action {
            check_easing(
                easing,
                None,
                format!("$.events[{i}].action.easing"),
                &mut errors,
                &mut warnings,
            );
        }
    }
    // Audio gain and start reach ffmpeg as filter-graph numbers. A non-finite
    // value formats as `inf` or `NaN`, which ffmpeg rejects — the whole export
    // fails with a parse error naming a filter the author never wrote.
    for (i, audio) in scene.assets.audio.iter().enumerate() {
        if !audio.gain.is_finite() || audio.gain < 0.0 {
            errors.push(ValidationError {
                code: "INVALID_AUDIO_GAIN".to_string(),
                path: format!("$.assets.audio[{i}].gain"),
                message: format!(
                    "audio gain is {}; it must be a finite, non-negative multiplier.",
                    audio.gain
                ),
                fix_suggestion: "Use 1.0 for the track as recorded, 0.5 to halve its                                  amplitude, or 0.0 to mute it."
                    .to_string(),
            });
        }
        if !audio.start.is_finite() {
            errors.push(ValidationError {
                code: "INVALID_AUDIO_START".to_string(),
                path: format!("$.assets.audio[{i}].start"),
                message: "audio start is not a finite number of seconds.".to_string(),
                fix_suggestion: "Use 0 to start with the video, a positive value to delay                                  the track, or a negative one to begin part-way into it."
                    .to_string(),
            });
        }
    }

    if let Some(camera) = &scene.camera {
        for (i, entry) in camera.timeline.iter().enumerate() {
            check_easing(
                &entry.easing,
                entry.easing_params.as_ref(),
                format!("$.camera.timeline[{i}].easing"),
                &mut errors,
                &mut warnings,
            );
            // A camera state is a root transform, so one non-finite component
            // makes the whole matrix non-finite and every object in the scene
            // disappears — a blank video with no diagnostic anywhere. JSON
            // cannot write `inf`, but a literal past f32's range becomes one
            // on the way into these fields.
            let st = &entry.state;
            for (name, v) in [
                ("x", st.x),
                ("y", st.y),
                ("zoom", st.zoom),
                ("rotation", st.rotation),
            ] {
                if !v.is_finite() {
                    errors.push(ValidationError {
                        code: "CAMERA_STATE_NOT_FINITE".to_string(),
                        path: format!("$.camera.timeline[{i}].state.{name}"),
                        message: format!(
                            "camera `{name}` is not a finite number. The camera transform is \
                             applied to every object, so the entire frame would render empty."
                        ),
                        fix_suggestion: format!(
                            "Give `{name}` a value within 32-bit float range (about \
                             ±3.4e38)."
                        ),
                    });
                }
            }
        }
    }

    ValidationResponse {
        valid: errors.is_empty(),
        errors,
        warnings,
    }
}

/// The LSF type name of an object, for diagnostics.
fn object_type_name(obj: &Object) -> &'static str {
    match obj {
        Object::Circle(_) => "Circle",
        Object::Rectangle(_) => "Rectangle",
        Object::Polygon(_) => "Polygon",
        Object::Path(_) => "Path",
        Object::Line(_) => "Line",
        Object::Arrow(_) => "Arrow",
        Object::Text(_) => "Text",
        Object::LaTeX(_) => "LaTeX",
        Object::MathML(_) => "MathML",
        Object::Image(_) => "Image",
        Object::SVG(_) => "SVG",
        Object::Group(_) => "Group",
        Object::NumberLine(_) => "NumberLine",
        Object::Axes(_) => "Axes",
        Object::Plot(_) => "Plot",
        Object::BezierCurve(_) => "BezierCurve",
        Object::Particles(_) => "Particles",
    }
}

/// Validate one easing reference: unknown names are errors with a
/// nearest-name suggestion; parameterized easings missing their params get
/// a warning (they fall back to a documented default at runtime).
fn check_easing(
    name: &str,
    params: Option<&serde_json::Value>,
    path: String,
    errors: &mut Vec<ValidationError>,
    warnings: &mut Vec<ValidationWarning>,
) {
    if !is_valid_easing(name) {
        let fix_suggestion = match suggest_easing(name) {
            Some(candidate) => format!("Did you mean '{candidate}'?"),
            None => "See luminafx_core::easing::EASING_NAMES for the accepted names.".to_string(),
        };
        errors.push(ValidationError {
            code: "UNKNOWN_EASING".to_string(),
            path,
            message: format!("Unknown easing '{name}'."),
            fix_suggestion,
        });
        return;
    }

    let params_ok = match name {
        "cubic_bezier" => params
            .and_then(|p| p.as_array())
            .is_some_and(|arr| arr.len() >= 4),
        "spline" => params
            .and_then(|p| p.get("keypoints"))
            .and_then(|k| k.as_array())
            .is_some_and(|kp| kp.len() >= 2),
        _ => true,
    };
    if !params_ok {
        warnings.push(ValidationWarning {
            code: "MISSING_EASING_PARAMS".to_string(),
            path,
            message: format!(
                "Easing '{}' needs easing_params ({}); without them it falls back to {}.",
                name,
                if name == "cubic_bezier" {
                    "[x1, y1, x2, y2]"
                } else {
                    "{\"keypoints\": [[t, v], …]} with ≥ 2 points"
                },
                if name == "cubic_bezier" {
                    "the CSS 'ease' curve"
                } else {
                    "linear"
                },
            ),
        });
        return;
    }

    // Present but invalid parameters are worse than missing ones: the solvers
    // assume preconditions the shapes above do not check, and violating them
    // yields a silently wrong curve rather than a fallback.
    match name {
        "cubic_bezier" => {
            if let Some(arr) = params.and_then(|p| p.as_array()) {
                let read = |i: usize| arr.get(i).and_then(serde_json::Value::as_f64);
                for (i, label) in [(0usize, "x1"), (2, "x2")] {
                    let Some(x) = read(i) else { continue };
                    // The solver inverts bezier_x by Newton with a bisection
                    // fallback, and both require x to be monotonic — which the
                    // CSS specification guarantees by constraining the x
                    // control points to [0, 1]. Outside it, the curve is not a
                    // function of time and the solver returns nonsense.
                    if !(0.0..=1.0).contains(&x) {
                        errors.push(ValidationError {
                            code: "INVALID_CUBIC_BEZIER".to_string(),
                            path: path.clone(),
                            message: format!(
                                "cubic_bezier {label} is {x}; the x control points must be within \
                                 [0, 1] or the curve is not a function of time."
                            ),
                            fix_suggestion: format!(
                                "Clamp {label} into [0, 1]. y values may fall outside it — that \
                                 is what produces overshoot."
                            ),
                        });
                    }
                }
                for (i, label) in [(0usize, "x1"), (1, "y1"), (2, "x2"), (3, "y2")] {
                    if let Some(v) = read(i) {
                        if !v.is_finite() {
                            errors.push(ValidationError {
                                code: "INVALID_CUBIC_BEZIER".to_string(),
                                path: path.clone(),
                                message: format!(
                                    "cubic_bezier {label} is {v}, which is not finite."
                                ),
                                fix_suggestion: "Use finite numbers for all four control points."
                                    .to_string(),
                            });
                        }
                    }
                }
            }
        }
        "spline" => {
            if let Some(kp) = params
                .and_then(|p| p.get("keypoints"))
                .and_then(|k| k.as_array())
            {
                let xs: Vec<f64> = kp
                    .iter()
                    .filter_map(|pair| pair.as_array()?.first()?.as_f64())
                    .collect();
                // The Fritsch-Carlson construction divides by the interval
                // between consecutive keypoints, floored at 1e-9. An unsorted
                // or duplicated pair therefore clamps a zero or negative
                // interval to 1e-9, producing tangents around 1e9 and garbage
                // output — which then becomes `null` via the non-finite path.
                if xs.windows(2).any(|w| w[1] <= w[0]) {
                    errors.push(ValidationError {
                        code: "UNSORTED_SPLINE_KEYPOINTS".to_string(),
                        path: path.clone(),
                        message: "spline keypoints must be sorted by time and strictly \
                                  increasing; equal or decreasing times make the interpolation \
                                  undefined."
                            .to_string(),
                        fix_suggestion: "Sort keypoints by their first element and remove \
                                         duplicate times."
                            .to_string(),
                    });
                }
                if xs.iter().any(|x| !x.is_finite()) {
                    errors.push(ValidationError {
                        code: "UNSORTED_SPLINE_KEYPOINTS".to_string(),
                        path,
                        message: "spline keypoint times must be finite.".to_string(),
                        fix_suggestion: "Replace any NaN or infinite keypoint time.".to_string(),
                    });
                }
            }
        }
        _ => {}
    }
}

/// Per-object limits on work the renderer repeats every frame.
///
/// Each of these was reachable from a request body of a few hundred bytes:
/// an unbounded `sample_count` or `count` multiplies by the frame count, and
/// a tick loop with a tiny or non-positive step multiplies by far more. The
/// `Axes` case additionally produced `inf as i32`, which saturates to
/// `i32::MAX` rather than failing.
fn validate_object_bounds(id: &str, obj: &Object, errors: &mut Vec<ValidationError>) {
    match obj {
        Object::Plot(p) => {
            if p.sample_count > MAX_PLOT_SAMPLES {
                errors.push(ValidationError {
                    code: "SAMPLE_COUNT_TOO_HIGH".to_string(),
                    path: format!("$.objects.{id}.properties.sample_count"),
                    message: format!(
                        "sample_count {} exceeds the maximum of {MAX_PLOT_SAMPLES}. The function \
                         is evaluated this many times per frame.",
                        p.sample_count
                    ),
                    fix_suggestion: format!(
                        "Reduce sample_count to at most {MAX_PLOT_SAMPLES}; a few hundred is \
                         usually indistinguishable from more."
                    ),
                });
            }
            if p.function_str.len() > MAX_EXPRESSION_BYTES {
                errors.push(ValidationError {
                    code: "EXPRESSION_TOO_LONG".to_string(),
                    path: format!("$.objects.{id}.properties.function_str"),
                    message: format!(
                        "function_str is {} bytes, over the maximum of {MAX_EXPRESSION_BYTES}. \
                         Expressions are parsed by recursive descent.",
                        p.function_str.len()
                    ),
                    fix_suggestion: "Simplify the expression, or precompute the curve and use a \
                                     Path object instead."
                        .to_string(),
                });
            }
        }
        Object::Particles(p) => {
            if p.count > MAX_PARTICLE_COUNT {
                errors.push(ValidationError {
                    code: "PARTICLE_COUNT_TOO_HIGH".to_string(),
                    path: format!("$.objects.{id}.properties.count"),
                    message: format!(
                        "count {} exceeds the maximum of {MAX_PARTICLE_COUNT}. Every particle is \
                         simulated and drawn each frame.",
                        p.count
                    ),
                    fix_suggestion: format!("Reduce count to at most {MAX_PARTICLE_COUNT}."),
                });
            }
        }
        Object::NumberLine(p) => {
            check_tick_count(
                id,
                "start/end/step",
                f64::from(p.start),
                f64::from(p.end),
                f64::from(p.step),
                errors,
            );
        }
        Object::Axes(p) => {
            check_tick_count(
                id,
                "x_step",
                f64::from(p.x_range[0]),
                f64::from(p.x_range[1]),
                f64::from(p.x_step),
                errors,
            );
            check_tick_count(
                id,
                "y_step",
                f64::from(p.y_range[0]),
                f64::from(p.y_range[1]),
                f64::from(p.y_step),
                errors,
            );
        }
        _ => {}
    }
}

/// A tick loop walks `min..=max` in increments of `step`, stroking a path each
/// time, on every frame. A non-positive or non-finite step never terminates
/// (or saturates a cast); a tiny one terminates far too late.
fn check_tick_count(
    id: &str,
    field: &str,
    min: f64,
    max: f64,
    step: f64,
    errors: &mut Vec<ValidationError>,
) {
    let path = format!("$.objects.{id}.properties.{field}");
    if !step.is_finite() || step <= 0.0 {
        errors.push(ValidationError {
            code: "INVALID_STEP".to_string(),
            path,
            message: format!(
                "{field} is {step}. Tick spacing must be a positive, finite number — a \
                 non-positive step describes a loop that never ends."
            ),
            fix_suggestion: "Set the step to a positive number (e.g. 1.0).".to_string(),
        });
        return;
    }
    if !min.is_finite() || !max.is_finite() {
        errors.push(ValidationError {
            code: "INVALID_RANGE".to_string(),
            path,
            message: format!("Range [{min}, {max}] must be finite."),
            fix_suggestion: "Set both range bounds to finite numbers.".to_string(),
        });
        return;
    }
    let ticks = (max - min).abs() / step;
    if ticks > MAX_TICK_COUNT {
        errors.push(ValidationError {
            code: "TOO_MANY_TICKS".to_string(),
            path,
            message: format!(
                "Range [{min}, {max}] with step {step} produces {ticks:.0} ticks, over the \
                 maximum of {MAX_TICK_COUNT:.0}. Each tick is drawn every frame."
            ),
            fix_suggestion: format!(
                "Increase the step, or narrow the range, so fewer than {MAX_TICK_COUNT:.0} ticks \
                 are produced."
            ),
        });
    }
}

/// What a group walk found: a reference cycle, or nesting too deep to walk.
enum GroupWalk {
    /// `g0 -> g1 -> ... -> g0`, reported so the message can name the loop.
    Cycle(Vec<String>),
    /// A chain longer than [`MAX_GROUP_DEPTH`]. Not a cycle — `visited` never
    /// trips on a straight chain — but recursing it would overflow the stack,
    /// which aborts the process rather than failing the request.
    TooDeep(String),
}

/// Reject numbers that cannot be represented as `f32`.
///
/// Recurses into arrays, since point lists and gradient stops are written that
/// way. `depth` bounds the walk; `serde_json` caps nesting at 128, so this is
/// belt and braces rather than a live vector.
fn check_representable(value: &Value, path: &str, errors: &mut Vec<ValidationError>, depth: usize) {
    const MAX_VALUE_DEPTH: usize = 32;
    if depth > MAX_VALUE_DEPTH {
        return;
    }
    match value {
        Value::Number(n) => {
            let Some(v) = n.as_f64() else { return };
            if !v.is_finite() || (v as f32).is_finite() {
                // Non-finite cannot appear in JSON, and anything that survives
                // the cast is fine.
                return;
            }
            errors.push(ValidationError {
                code: "NUMBER_NOT_REPRESENTABLE".to_string(),
                path: path.to_string(),
                message: format!(
                    "{v:e} overflows 32-bit float, which is the precision the engine renders \
                     with. The property would silently vanish rather than animate."
                ),
                fix_suggestion: format!(
                    "Use a value within +/-{:e}. Coordinates this large are almost always a \
                     generated-scene bug.",
                    f32::MAX
                ),
            });
        }
        Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                check_representable(item, &format!("{path}[{i}]"), errors, depth + 1);
            }
        }
        _ => {}
    }
}

/// Reject colour strings the renderer cannot parse.
///
/// `parse_rgba8` falls back to opaque white for anything it does not
/// recognise, so `"#GGG"`, `"red"`, or SVG's `"none"` all render as a white
/// shape and nothing reports it.
fn check_colour(value: &str, path: &str, errors: &mut Vec<ValidationError>) {
    let hex = value.strip_prefix('#').unwrap_or("");
    let ok = matches!(hex.len(), 3 | 6 | 8) && hex.chars().all(|c| c.is_ascii_hexdigit());
    if ok {
        return;
    }
    errors.push(ValidationError {
        code: "INVALID_COLOR".to_string(),
        path: path.to_string(),
        message: format!(
            "{value:?} is not a colour. The renderer would draw it as opaque white without \
             reporting anything."
        ),
        fix_suggestion: if value.eq_ignore_ascii_case("none") {
            "There is no \"none\" value. Omit `fill` for a shape with no fill, or use an \
             alpha of 00 (e.g. \"#00000000\")."
                .to_string()
        } else {
            "Use #RGB, #RRGGBB, or #RRGGBBAA.".to_string()
        },
    });
}

fn detect_group_cycle(objects: &HashMap<String, Object>) -> Option<GroupWalk> {
    fn dfs<'a>(
        id: &'a str,
        objects: &'a HashMap<String, Object>,
        visited: &mut HashSet<&'a str>,
        path: &mut Vec<&'a str>,
    ) -> Option<GroupWalk> {
        if let Some(start) = path.iter().position(|x| *x == id) {
            let mut cycle: Vec<String> = path[start..].iter().map(|s| (*s).to_string()).collect();
            cycle.push(id.to_string());
            return Some(GroupWalk::Cycle(cycle));
        }
        if path.len() >= MAX_GROUP_DEPTH {
            return Some(GroupWalk::TooDeep(id.to_string()));
        }
        if visited.contains(id) {
            return None;
        }
        visited.insert(id);
        if let Some(Object::Group(group)) = objects.get(id) {
            path.push(id);
            for child in &group.children {
                if let Some(found) = dfs(child, objects, visited, path) {
                    return Some(found);
                }
            }
            path.pop();
        }
        None
    }

    let mut visited = HashSet::new();
    for id in objects.keys() {
        let mut path = Vec::new();
        if let Some(found) = dfs(id, objects, &mut visited, &mut path) {
            return Some(found);
        }
    }
    None
}
