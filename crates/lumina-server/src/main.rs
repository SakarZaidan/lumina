use axum::{
    extract::Json,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use lumina_export::Exporter;
use lumina_renderer::skia_backend::SkiaRenderer;
use lumina_schema::{Object, Scene};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;

#[derive(Debug, Serialize, Deserialize)]
struct RenderRequest {
    scene: Scene,
    #[serde(default = "default_format")]
    format: String,
}

fn default_format() -> String {
    "mp4".to_string()
}

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

    // --- Check 1: Timeline entries must reference declared object IDs ---
    for (i, entry) in scene.timeline.iter().enumerate() {
        if !object_ids.contains(entry.object.as_str()) {
            // Find closest match by prefix for a helpful suggestion
            let suggestion = object_ids.iter()
                .filter(|id| id.starts_with(&entry.object[..entry.object.len().min(3)]))
                .next()
                .map(|s| format!("Did you mean '{}'?", s))
                .unwrap_or_else(|| "Check the 'objects' block for valid IDs.".to_string());

            errors.push(ValidationError {
                code: "UNKNOWN_OBJECT_ID".to_string(),
                path: format!("$.timeline[{}].object", i),
                message: format!(
                    "Timeline entry at index {} references object '{}', but no such object exists in the 'objects' block.",
                    i, entry.object
                ),
                fix_suggestion: suggestion,
            });
        }
    }

    // --- Check 2: Event entries must reference declared object IDs ---
    for (i, event) in scene.events.iter().enumerate() {
        if !object_ids.contains(event.object.as_str()) {
            errors.push(ValidationError {
                code: "UNKNOWN_OBJECT_ID".to_string(),
                path: format!("$.events[{}].object", i),
                message: format!(
                    "Event at index {} references object '{}', which is not declared.",
                    i, event.object
                ),
                fix_suggestion: format!(
                    "Add '{}' to the 'objects' block or correct the event's object field.",
                    event.object
                ),
            });
        }
    }

    // --- Check 3: Group children must reference declared object IDs ---
    for (obj_id, obj) in &scene.objects {
        if let Object::Group(group) = obj {
            for child_id in &group.children {
                if !object_ids.contains(child_id.as_str()) {
                    errors.push(ValidationError {
                        code: "UNKNOWN_CHILD_ID".to_string(),
                        path: format!("$.objects.{}.properties.children", obj_id),
                        message: format!(
                            "Group '{}' references child '{}', which is not declared in the 'objects' block.",
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

    // --- Check 4: Circular group references ---
    if let Some(cycle) = detect_group_cycle(&scene.objects) {
        errors.push(ValidationError {
            code: "CIRCULAR_GROUP_REFERENCE".to_string(),
            path: "$.objects".to_string(),
            message: format!(
                "Circular group reference detected involving: {}. Groups cannot contain themselves.",
                cycle.join(" → ")
            ),
            fix_suggestion: "Remove the circular dependency from the group's children list.".to_string(),
        });
    }

    // --- Check 5: Timeline entries beyond canvas duration (warning) ---
    for (i, entry) in scene.timeline.iter().enumerate() {
        if entry.time > scene.canvas.duration {
            warnings.push(ValidationWarning {
                code: "KEYFRAME_BEYOND_DURATION".to_string(),
                path: format!("$.timeline[{}].time", i),
                message: format!(
                    "Keyframe at index {} has time={:.2}s but canvas duration is {:.2}s. This keyframe will never be reached.",
                    i, entry.time, scene.canvas.duration
                ),
            });
        }
    }

    // --- Check 6: Duplicate keyframes (same object + property + time) — warning ---
    // Group timeline entries by (object, property, time)
    let mut seen: HashMap<(String, String, String), usize> = HashMap::new();
    for (i, entry) in scene.timeline.iter().enumerate() {
        if let serde_json::Value::Object(state) = &entry.state {
            for prop_name in state.keys() {
                let key = (entry.object.clone(), prop_name.clone(), format!("{:.6}", entry.time));
                if let Some(first_idx) = seen.get(&key) {
                    warnings.push(ValidationWarning {
                        code: "DUPLICATE_KEYFRAME".to_string(),
                        path: format!("$.timeline[{}]", i),
                        message: format!(
                            "Duplicate keyframe for object '{}' property '{}' at t={:.2}s (first seen at index {}). Last declaration wins.",
                            entry.object, prop_name, entry.time, first_idx
                        ),
                    });
                } else {
                    seen.insert(key, i);
                }
            }
        }
    }

    // --- Check 7: Canvas dimensions must be positive ---
    if scene.canvas.width == 0 || scene.canvas.height == 0 {
        errors.push(ValidationError {
            code: "INVALID_CANVAS_SIZE".to_string(),
            path: "$.canvas".to_string(),
            message: format!(
                "Canvas size {}x{} is invalid. Both width and height must be > 0.",
                scene.canvas.width, scene.canvas.height
            ),
            fix_suggestion: "Set canvas.width and canvas.height to positive integers (e.g. 1280 and 720).".to_string(),
        });
    }

    // --- Check 8: Large scene warning ---
    if scene.objects.len() > 500 {
        warnings.push(ValidationWarning {
            code: "LARGE_SCENE".to_string(),
            path: "$.objects".to_string(),
            message: format!(
                "Scene has {} objects. Consider grouping related objects to improve render performance.",
                scene.objects.len()
            ),
        });
    }

    ValidationResponse {
        valid: errors.is_empty(),
        errors,
        warnings,
    }
}

/// Returns a cycle description if any group contains itself transitively, None otherwise.
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

async fn validate_scene(Json(scene): Json<Scene>) -> impl IntoResponse {
    Json(validate_scene_data(&scene))
}

async fn render_scene(Json(payload): Json<RenderRequest>) -> Response {
    // Validate before rendering
    let validation = validate_scene_data(&payload.scene);
    if !validation.valid {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(validation),
        ).into_response();
    }

    let renderer = SkiaRenderer::new();
    let mut exporter = Exporter::new(renderer);

    let temp_dir = match tempfile::tempdir() {
        Ok(d) => d,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Temp dir error: {}", e)).into_response(),
    };
    let output_path = temp_dir.path().join("output.mp4");

    match exporter.export_mp4(&payload.scene, &output_path) {
        Ok(_) => {
            match std::fs::read(&output_path) {
                Ok(data) => Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", "video/mp4")
                    .body(axum::body::Body::from(data))
                    .unwrap(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to read output: {}", e)).into_response(),
            }
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Rendering failed: {}", e)).into_response(),
    }
}

async fn health_check() -> &'static str {
    "Lumina Server is online"
}

#[tokio::main]
async fn main() {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/validate", post(validate_scene))
        .route("/render", post(render_scene))
        .layer(CorsLayer::permissive());

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    log::info!("Lumina Server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use lumina_schema::{Canvas, CircleProps, GroupProps, Meta, TimelineEntry};

    fn minimal_scene() -> Scene {
        Scene {
            version: "1.0".into(),
            meta: Meta { title: "Test".into(), author: "test".into(), created_at: "2026-01-01T00:00:00Z".into() },
            canvas: Canvas { width: 100, height: 100, fps: 30, duration: 2.0, background: "#000000".into() },
            assets: Default::default(),
            objects: {
                let mut m = HashMap::new();
                m.insert("circle".into(), Object::Circle(CircleProps {
                    cx: 50.0, cy: 50.0, radius: 20.0,
                    z_index: 0, fill: "#FF0000".into(), stroke: None, stroke_width: 0.0, opacity: 1.0,
                }));
                m
            },
            timeline: vec![],
            events: vec![],
            camera: None,
        }
    }

    #[test]
    fn test_valid_scene_passes() {
        let scene = minimal_scene();
        let result = validate_scene_data(&scene);
        assert!(result.valid, "Expected valid scene to pass, got errors: {:?}", result.errors);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_unknown_object_id_in_timeline() {
        let mut scene = minimal_scene();
        scene.timeline.push(TimelineEntry {
            time: 1.0,
            object: "nonexistent".into(),
            state: serde_json::json!({"opacity": 1.0}),
            easing: "linear".into(),
            easing_params: None,
        });
        let result = validate_scene_data(&scene);
        assert!(!result.valid);
        assert!(result.errors.iter().any(|e| e.code == "UNKNOWN_OBJECT_ID"));
        assert!(result.errors.iter().any(|e| e.path.contains("timeline[0]")));
    }

    #[test]
    fn test_circular_group_reference_detected() {
        let mut scene = minimal_scene();
        scene.objects.insert("group_a".into(), Object::Group(GroupProps {
            children: vec!["group_b".into()],
            x: 0.0, y: 0.0, z_index: 0, scale: 1.0, rotation: 0.0, opacity: 1.0,
        }));
        scene.objects.insert("group_b".into(), Object::Group(GroupProps {
            children: vec!["group_a".into()],
            x: 0.0, y: 0.0, z_index: 0, scale: 1.0, rotation: 0.0, opacity: 1.0,
        }));
        let result = validate_scene_data(&scene);
        assert!(!result.valid);
        assert!(result.errors.iter().any(|e| e.code == "CIRCULAR_GROUP_REFERENCE"));
    }

    #[test]
    fn test_keyframe_beyond_duration_is_warning() {
        let mut scene = minimal_scene();
        scene.timeline.push(TimelineEntry {
            time: 999.0, // way past duration=2.0
            object: "circle".into(),
            state: serde_json::json!({"opacity": 1.0}),
            easing: "linear".into(),
            easing_params: None,
        });
        let result = validate_scene_data(&scene);
        assert!(result.valid, "Beyond-duration keyframe should be a warning, not an error");
        assert!(result.warnings.iter().any(|w| w.code == "KEYFRAME_BEYOND_DURATION"));
    }

    #[test]
    fn test_zero_canvas_size_is_error() {
        let mut scene = minimal_scene();
        scene.canvas.width = 0;
        let result = validate_scene_data(&scene);
        assert!(!result.valid);
        assert!(result.errors.iter().any(|e| e.code == "INVALID_CANVAS_SIZE"));
    }

    #[test]
    fn test_duplicate_keyframe_is_warning() {
        let mut scene = minimal_scene();
        let kf = TimelineEntry {
            time: 1.0,
            object: "circle".into(),
            state: serde_json::json!({"opacity": 0.5}),
            easing: "linear".into(),
            easing_params: None,
        };
        scene.timeline.push(kf.clone());
        scene.timeline.push(kf);
        let result = validate_scene_data(&scene);
        assert!(result.valid, "Duplicate keyframe should warn, not error");
        assert!(result.warnings.iter().any(|w| w.code == "DUPLICATE_KEYFRAME"));
    }
}
