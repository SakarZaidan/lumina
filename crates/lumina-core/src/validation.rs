//! Semantic scene validation.
//!
//! Lives in `lumina-core` so every consumer — server, CLI, SDKs — applies
//! the same rules; `lumina-server` re-exports these types unchanged.
//! Errors are render-blocking; warnings are advisory. `fix_suggestion` is
//! written for self-correction loops (both human and LLM authors).

use crate::easing::{is_valid_easing, suggest_easing};
use lumina_schema::{Action, Object, Scene};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Serialize, Clone)]
pub struct ValidationResponse {
    pub valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ValidationError {
    pub code: String,
    pub path: String,
    pub message: String,
    pub fix_suggestion: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct ValidationWarning {
    pub code: String,
    pub path: String,
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
            let suggestion = object_ids
                .iter()
                .find(|id| id.starts_with(&entry.object[..entry.object.len().min(3)]))
                .map(|s| format!("Did you mean '{}'?", s))
                .unwrap_or_else(|| "Check the 'objects' block for valid IDs.".to_string());

            errors.push(ValidationError {
                code: "UNKNOWN_OBJECT_ID".to_string(),
                path: format!("$.timeline[{}].object", i),
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
                path: format!("$.events[{}].object", i),
                message: format!(
                    "Event {} references object '{}', which is not declared.",
                    i, event.object
                ),
                fix_suggestion: format!(
                    "Add '{}' to the 'objects' block or correct the event's object field.",
                    event.object
                ),
            });
        }
    }

    // Check 3: Group children must reference declared object IDs
    for (obj_id, obj) in &scene.objects {
        if let Object::Group(group) = obj {
            for child_id in &group.children {
                if !object_ids.contains(child_id.as_str()) {
                    errors.push(ValidationError {
                        code: "UNKNOWN_CHILD_ID".to_string(),
                        path: format!("$.objects.{}.properties.children", obj_id),
                        message: format!(
                            "Group '{}' references child '{}', which is not declared.",
                            obj_id, child_id
                        ),
                        fix_suggestion: format!(
                            "Add '{}' to the 'objects' block or remove it from group '{}'.",
                            child_id, obj_id
                        ),
                    });
                }
            }
        }
    }

    // Check 4: Circular group references
    if let Some(cycle) = detect_group_cycle(&scene.objects) {
        errors.push(ValidationError {
            code: "CIRCULAR_GROUP_REFERENCE".to_string(),
            path: "$.objects".to_string(),
            message: format!(
                "Circular group reference: {}. Groups cannot contain themselves.",
                cycle.join(" → ")
            ),
            fix_suggestion: "Remove the circular dependency from the group's children list."
                .to_string(),
        });
    }

    // Check 5: Keyframes beyond canvas duration (warning)
    for (i, entry) in scene.timeline.iter().enumerate() {
        if entry.time > scene.canvas.duration {
            warnings.push(ValidationWarning {
                code: "KEYFRAME_BEYOND_DURATION".to_string(),
                path: format!("$.timeline[{}].time", i),
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
                        path: format!("$.timeline[{}]", i),
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
            format!("$.timeline[{}].easing", i),
            &mut errors,
            &mut warnings,
        );
    }
    for (i, event) in scene.events.iter().enumerate() {
        if let Action::TweenTo { easing, .. } = &event.action {
            check_easing(
                easing,
                None,
                format!("$.events[{}].action.easing", i),
                &mut errors,
                &mut warnings,
            );
        }
    }
    if let Some(camera) = &scene.camera {
        for (i, entry) in camera.timeline.iter().enumerate() {
            check_easing(
                &entry.easing,
                None,
                format!("$.camera.timeline[{}].easing", i),
                &mut errors,
                &mut warnings,
            );
        }
    }

    ValidationResponse {
        valid: errors.is_empty(),
        errors,
        warnings,
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
            Some(candidate) => format!("Did you mean '{}'?", candidate),
            None => "See lumina_core::easing::EASING_NAMES for the accepted names.".to_string(),
        };
        errors.push(ValidationError {
            code: "UNKNOWN_EASING".to_string(),
            path,
            message: format!("Unknown easing '{}'.", name),
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
    }
}

fn detect_group_cycle(objects: &HashMap<String, Object>) -> Option<Vec<String>> {
    fn dfs(
        id: &str,
        objects: &HashMap<String, Object>,
        visited: &mut HashSet<String>,
        path: &mut Vec<String>,
    ) -> Option<Vec<String>> {
        if path.contains(&id.to_string()) {
            let start = path.iter().position(|x| x == id).unwrap();
            let mut cycle = path[start..].to_vec();
            cycle.push(id.to_string());
            return Some(cycle);
        }
        if visited.contains(id) {
            return None;
        }
        visited.insert(id.to_string());
        if let Some(Object::Group(group)) = objects.get(id) {
            path.push(id.to_string());
            for child in &group.children {
                if let Some(cycle) = dfs(child, objects, visited, path) {
                    return Some(cycle);
                }
            }
            path.pop();
        }
        None
    }

    let mut visited = HashSet::new();
    for id in objects.keys() {
        let mut path = Vec::new();
        if let Some(cycle) = dfs(id, objects, &mut visited, &mut path) {
            return Some(cycle);
        }
    }
    None
}
