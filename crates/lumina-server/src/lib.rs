//! HTTP API server for the Lumina animation engine.
//!
//! An `axum` REST service designed for AI/authoring loops:
//!
//! - `GET /health`, `GET /schema` (JSON Schema of the scene format),
//!   `GET /objects` (object-type registry for introspection)
//! - `POST /validate` — structural validation with structured errors, each
//!   carrying a `code`, `path`, and machine-actionable `fix_suggestion`
//! - `POST /patch` (RFC-6902) and `POST /scene_patch` (semantic ops)
//! - `POST /render` — renders the posted scene and returns MP4/WebM/GIF bytes
//!
//! **Security note:** requests are capped at 8 MiB, `/render` asset paths
//! are restricted to `LUMINA_ASSET_ROOT` (default: the working directory),
//! and bind/serve failures return errors instead of panicking. The server
//! is still not hardened for untrusted networks — no authentication, rate
//! limiting, or CORS allowlist until v0.5 (see `SECURITY.md` and
//! `planning/TECH_DEBT.md` TD-09). Run it locally or behind a trusted
//! reverse proxy.

// The engine has never contained `unsafe`, and the metric tracking that was a
// `grep` over the source — which by v0.4.0 was returning a false positive from
// the word appearing in a comment. `forbid` makes it a compile error instead:
// it cannot be silenced by an `allow` further down, so a future `unsafe` block
// has to be argued for by removing this line, in a diff a reviewer will see.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

use axum::{
    extract::Json,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use lumina_export::Exporter;
use lumina_renderer::skia_backend::SkiaRenderer;
use lumina_renderer::Renderer;
use lumina_schema::Scene;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;

// ── Request / Response types ──────────────────────────────────────────────────

/// Body of `POST /render`: the scene plus the requested container format.
#[derive(Debug, Serialize, Deserialize)]
pub struct RenderRequest {
    /// The scene to render.
    pub scene: Scene,
    /// Output container: `"mp4"` (default), `"webm"`, or `"gif"`.
    #[serde(default = "default_format")]
    pub format: String,
}

fn default_format() -> String {
    "mp4".to_string()
}

/// Body of `POST /patch`: a scene document plus RFC-6902 JSON Patch ops.
#[derive(Debug, Serialize, Deserialize)]
pub struct PatchRequest {
    /// The scene as a raw JSON document (patched before typing).
    pub scene: serde_json::Value,
    /// RFC 6902 operations to apply, in order.
    pub patch: Vec<serde_json::Value>,
}

/// Response of the patch endpoints: the updated scene and its validation.
#[derive(Debug, Serialize, Clone)]
pub struct PatchResponse {
    /// The scene after the patch was applied.
    pub scene: serde_json::Value,
    /// Validation results for the patched scene.
    pub validation: ValidationResponse,
}

/// Semantic patch request: a typed scene plus a list of domain-level operations
/// (`add_object`, `add_keyframe`, `update_canvas`, …) applied via `lumina_core`.
#[derive(Debug, Deserialize)]
pub struct ScenePatchRequest {
    /// The scene to patch.
    pub scene: Scene,
    /// Semantic operations to apply, in order.
    pub patch: lumina_core::ScenePatch,
}

// ── Semantic validation ───────────────────────────────────────────────────────
//
// Validation logic lives in lumina-core (shared by server, CLI and SDKs);
// re-exported here so the server's public API is unchanged.
pub use lumina_core::validation::{
    validate_scene_data, ValidationError, ValidationResponse, ValidationWarning,
};

// ── Route handlers ────────────────────────────────────────────────────────────

async fn health_check() -> &'static str {
    "Lumina OK"
}

async fn validate_scene(Json(scene): Json<Scene>) -> impl IntoResponse {
    Json(validate_scene_data(&scene))
}

/// `GET /schema` — returns the LSF JSON Schema derived from the Rust types.
async fn get_schema() -> impl IntoResponse {
    let schema = schemars::schema_for!(Scene);
    Json(schema)
}

/// `GET /objects` — returns the object-type registry: for each LSF object type,
/// its required and optional properties. Intended for LLM/agent introspection.
async fn get_objects() -> impl IntoResponse {
    Json(object_registry())
}

fn object_registry() -> serde_json::Value {
    use serde_json::json;
    json!({
        "Circle":      { "required": ["cx", "cy", "radius"], "optional": ["fill", "stroke", "stroke_width", "shadow", "opacity", "z_index"] },
        "Rectangle":   { "required": ["x", "y", "width", "height"], "optional": ["fill", "stroke", "stroke_width", "rx", "ry", "shadow", "opacity", "z_index"] },
        "Polygon":     { "required": ["points"], "optional": ["fill", "stroke", "stroke_width", "shadow", "opacity", "z_index"] },
        "Path":        { "required": ["d"], "optional": ["fill", "stroke", "stroke_width", "draw_fraction", "shadow", "opacity", "z_index"] },
        "Line":        { "required": ["x1", "y1", "x2", "y2"], "optional": ["stroke", "stroke_width", "dash", "draw_fraction", "opacity", "z_index"] },
        "Arrow":       { "required": ["from", "to"], "optional": ["color", "stroke_width", "label", "opacity", "z_index"] },
        "Text":        { "required": ["content", "x", "y", "font_size"], "optional": ["font_id", "color", "align", "letter_spacing", "opacity", "z_index"] },
        "LaTeX":       { "required": ["expression", "x", "y", "font_size"], "optional": ["color", "draw_fraction", "align", "letter_spacing", "opacity", "z_index"] },
        "MathML":      { "required": ["markup", "x", "y", "font_size"], "optional": ["color", "align", "letter_spacing", "opacity", "z_index"] },
        "Image":       { "required": ["asset_id", "x", "y"], "optional": ["width", "height", "rotation", "opacity", "z_index"] },
        "SVG":         { "required": ["asset_id", "x", "y"], "optional": ["width", "height", "rotation", "opacity", "z_index"] },
        "Group":       { "required": ["children", "x", "y"], "optional": ["scale", "rotation", "opacity", "z_index"] },
        "NumberLine":  { "required": ["start", "end", "step", "x", "y"], "optional": ["length", "color", "opacity", "z_index"] },
        "Axes":        { "required": ["x_range", "y_range", "x", "y"], "optional": ["scale", "x_step", "y_step", "x_label", "y_label", "grid", "color", "opacity", "z_index"] },
        "Plot":        { "required": ["function_str", "axes_id"], "optional": ["color", "stroke_width", "sample_count", "draw_fraction", "opacity", "z_index"] },
        "BezierCurve": { "required": ["p0", "p1", "p2", "p3"], "optional": ["stroke", "stroke_width", "draw_fraction", "opacity", "z_index"] },
        "Particles":   { "required": ["count", "emitter_x", "emitter_y"], "optional": ["lifetime", "speed", "spread", "size", "color", "opacity", "z_index"] }
    })
}

/// `POST /patch` — applies a JSON Patch (RFC 6902) to a scene value, then
/// re-validates and returns the updated scene together with validation results.
async fn patch_scene(Json(payload): Json<PatchRequest>) -> Response {
    let patch_ops: json_patch::Patch =
        match serde_json::from_value(serde_json::Value::Array(payload.patch)) {
            Ok(p) => p,
            Err(e) => {
                return (StatusCode::BAD_REQUEST, format!("Invalid JSON Patch: {e}"))
                    .into_response()
            }
        };

    let mut scene_value = payload.scene;
    if let Err(e) = json_patch::patch(&mut scene_value, &patch_ops) {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("Patch failed: {e}"),
        )
            .into_response();
    }

    let scene: Scene = match serde_json::from_value(scene_value.clone()) {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("Patched document is not a valid Scene: {e}"),
            )
                .into_response()
        }
    };

    let validation = validate_scene_data(&scene);
    Json(PatchResponse {
        scene: scene_value,
        validation,
    })
    .into_response()
}

async fn scene_patch(Json(mut payload): Json<ScenePatchRequest>) -> Response {
    if let Err(e) = lumina_core::apply_patch(&mut payload.scene, &payload.patch) {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("Scene patch failed: {e}"),
        )
            .into_response();
    }
    let validation = validate_scene_data(&payload.scene);
    let scene_value = match serde_json::to_value(&payload.scene) {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Serialize error: {e}"),
            )
                .into_response()
        }
    };
    Json(PatchResponse {
        scene: scene_value,
        validation,
    })
    .into_response()
}

/// Resolve a scene-declared asset path against the allowed asset root.
///
/// The root is `LUMINA_ASSET_ROOT` (default: the server's working
/// directory). Canonicalization resolves `..` and symlinks before the
/// prefix check, so traversal cannot escape the root. Returns the resolved
/// path or a client-safe error message (no filesystem details).
fn resolve_asset_path(requested: &str) -> Result<std::path::PathBuf, String> {
    let root = std::env::var_os("LUMINA_ASSET_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let root = root
        .canonicalize()
        .map_err(|_| "asset root is not accessible".to_string())?;
    let candidate = if std::path::Path::new(requested).is_absolute() {
        std::path::PathBuf::from(requested)
    } else {
        root.join(requested)
    };
    let resolved = candidate
        .canonicalize()
        .map_err(|_| format!("asset '{requested}' not found under the asset root"))?;
    if resolved.starts_with(&root) {
        Ok(resolved)
    } else {
        Err(format!("asset '{requested}' is outside the asset root"))
    }
}

/// What a blocking render produced, or why it could not.
///
/// The render runs on a blocking thread, so it cannot build an axum `Response`
/// (which is not `Send` across every branch here). It returns this instead and
/// the async handler maps it, which also keeps status-code policy in one place.
enum RenderOutcome {
    Done {
        bytes: Vec<u8>,
    },
    /// The caller asked for something invalid — an asset path outside the root.
    BadRequest(String),
    /// Something failed on our side.
    Failed(String),
}

async fn render_scene(Json(payload): Json<RenderRequest>) -> Response {
    // Validation is cheap and bounded (see AAA-SEC-01), so it stays on the
    // async path: an invalid scene is rejected without occupying a blocking
    // thread at all.
    let validation = validate_scene_data(&payload.scene);
    if !validation.valid {
        return (StatusCode::UNPROCESSABLE_ENTITY, Json(validation)).into_response();
    }

    let (ext, content_type) = match payload.format.as_str() {
        "webm" => ("webm", "video/webm"),
        "gif" => ("gif", "image/gif"),
        _ => ("mp4", "video/mp4"),
    };

    // Everything past here is CPU-bound rendering followed by a blocking wait
    // on ffmpeg. Running it directly on the async path let N concurrent
    // renders (N = worker threads) starve the runtime, so /health and every
    // other route stopped answering until they finished.
    let outcome = tokio::task::spawn_blocking(move || render_blocking(&payload, ext)).await;

    match outcome {
        Ok(RenderOutcome::Done { bytes }) => Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", content_type)
            .body(axum::body::Body::from(bytes))
            .unwrap_or_else(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Response build error: {e}"),
                )
                    .into_response()
            }),
        Ok(RenderOutcome::BadRequest(msg)) => (StatusCode::BAD_REQUEST, msg).into_response(),
        Ok(RenderOutcome::Failed(msg)) => (StatusCode::INTERNAL_SERVER_ERROR, msg).into_response(),
        // The blocking task panicked or was cancelled. Nothing in it should
        // panic, but a 500 is the honest answer if one ever does.
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Render task did not complete: {e}"),
        )
            .into_response(),
    }
}

/// Load assets, render, and encode. Runs on a blocking thread.
fn render_blocking(payload: &RenderRequest, ext: &str) -> RenderOutcome {
    let mut renderer = SkiaRenderer::new();
    // Load declared font/image assets from disk, restricted to the asset
    // root (LUMINA_ASSET_ROOT, default CWD): /render must not be a remote
    // arbitrary-file read. Paths escaping the root fail the request;
    // in-root but unreadable/undecodable assets are logged and skipped.
    for font in &payload.scene.assets.fonts {
        let path = match resolve_asset_path(&font.path) {
            Ok(p) => p,
            Err(msg) => return RenderOutcome::BadRequest(msg),
        };
        match std::fs::read(&path) {
            Ok(data) => {
                if let Err(e) = renderer.load_font(&font.id, &data) {
                    log::warn!("Skipping font '{}': {:?}", font.id, e);
                }
            }
            Err(e) => log::warn!("Cannot read font '{}' at {:?}: {}", font.id, path, e),
        }
    }
    for img in &payload.scene.assets.images {
        let path = match resolve_asset_path(&img.path) {
            Ok(p) => p,
            Err(msg) => return RenderOutcome::BadRequest(msg),
        };
        match std::fs::read(&path) {
            Ok(data) => {
                if let Err(e) = renderer.load_image(&img.id, &data) {
                    log::warn!("Skipping image '{}': {:?}", img.id, e);
                }
            }
            Err(e) => log::warn!("Cannot read image '{}' at {:?}: {}", img.id, path, e),
        }
    }
    let mut exporter = Exporter::new(renderer);

    let temp_dir = match tempfile::tempdir() {
        Ok(d) => d,
        Err(e) => return RenderOutcome::Failed(format!("Temp dir error: {e}")),
    };
    let output_path = temp_dir.path().join(format!("output.{ext}"));

    let result = match ext {
        "webm" => exporter.export_webm(&payload.scene, &output_path),
        "gif" => exporter.export_gif(&payload.scene, &output_path),
        _ => exporter.export_mp4(&payload.scene, &output_path),
    };

    match result {
        Ok(_) => match std::fs::read(&output_path) {
            Ok(bytes) => RenderOutcome::Done { bytes },
            Err(e) => RenderOutcome::Failed(format!("Failed to read output: {e}")),
        },
        Err(e) => RenderOutcome::Failed(format!("Rendering failed: {e}")),
    }
}

// ── Router + server entry point ───────────────────────────────────────────────

/// Maximum accepted request body: scenes are JSON of at most a few MB;
/// anything larger is rejected up front (413) instead of being buffered.
const MAX_BODY_BYTES: usize = 8 * 1024 * 1024;

/// The full route table (health, schema, objects, validate, patch,
/// `scene_patch`, render) with the body-size and CORS layers applied.
pub fn build_router() -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/schema", get(get_schema))
        .route("/objects", get(get_objects))
        .route("/validate", post(validate_scene))
        .route("/patch", post(patch_scene))
        .route("/scene_patch", post(scene_patch))
        .route("/render", post(render_scene))
        .layer(axum::extract::DefaultBodyLimit::max(MAX_BODY_BYTES))
        .layer(CorsLayer::permissive())
}

/// Bind and serve until shutdown. Returns instead of panicking when the
/// port is taken or the listener dies, so embedders can report the error.
pub async fn run_server() -> Result<(), std::io::Error> {
    let app = build_router();
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    log::info!("Lumina Server listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use lumina_schema::{Canvas, CircleProps, GroupProps, Meta, Object, TimelineEntry};
    use std::collections::HashMap;

    /// The render path must not occupy the async runtime.
    ///
    /// `render_scene` used to render every frame and then block on ffmpeg
    /// directly on a worker thread. With as many concurrent renders as there
    /// are workers, the runtime had nothing left to schedule and `/health` —
    /// which does no work at all — stopped answering.
    ///
    /// This is asserted structurally rather than by timing. A timing test was
    /// written first and rejected: it passed against the *unfixed* handler,
    /// because starving a runtime deterministically needs a render slow enough
    /// to make the suite slow, and anything faster is a coin flip. A test that
    /// passes without the fix is worse than no test.
    ///
    /// The same technique guards backend deduplication in
    /// `lumina-renderer/tests/duplication_gate.rs`.
    #[test]
    fn render_runs_off_the_async_runtime() {
        let src = include_str!("lib.rs");
        let handler = src
            .split("async fn render_scene")
            .nth(1)
            .expect("render_scene must exist");
        // Stop at the next item so we only inspect this handler's body.
        let body = handler
            .split("\nfn render_blocking")
            .next()
            .unwrap_or(handler);

        assert!(
            body.contains("spawn_blocking"),
            "render_scene must hand its work to tokio::task::spawn_blocking. Rendering is \
             CPU-bound and then blocks on ffmpeg; on the async path it starves every worker \
             thread and the whole server stops responding."
        );
        assert!(
            !body.contains("exporter.export_"),
            "render_scene must not call an exporter directly — that work belongs on a blocking \
             thread, in render_blocking."
        );
    }

    #[test]
    fn asset_paths_inside_root_resolve() {
        // CWD is the crate root during tests; Cargo.toml is a real in-root file.
        assert!(resolve_asset_path("Cargo.toml").is_ok());
    }

    #[test]
    fn asset_paths_escaping_root_are_rejected() {
        assert!(resolve_asset_path("../../etc/passwd").is_err());
        assert!(resolve_asset_path("/etc/passwd").is_err());
        assert!(resolve_asset_path("../lumina-core/Cargo.toml").is_err());
    }

    fn minimal_scene() -> Scene {
        Scene {
            version: "1.0".into(),
            meta: Meta {
                title: "Test".into(),
                author: "test".into(),
                created_at: "2026-01-01T00:00:00Z".into(),
            },
            canvas: Canvas {
                width: 100,
                height: 100,
                fps: 30,
                duration: 2.0,
                background: "#000000".into(),
                motion_blur_samples: 1,
                shutter: 0.5,
            },
            assets: Default::default(),
            objects: {
                let mut m = HashMap::new();
                m.insert(
                    "circle".into(),
                    Object::Circle(CircleProps {
                        cx: 50.0,
                        cy: 50.0,
                        radius: 20.0,
                        z_index: 0,
                        fill: "#FF0000".into(),
                        stroke: None,
                        stroke_width: 0.0,
                        shadow: None,
                        opacity: 1.0,
                    }),
                );
                m
            },
            timeline: vec![],
            events: vec![],
            camera: None,
        }
    }

    #[test]
    fn test_scene_patch_add_object_then_validates() {
        let mut scene = minimal_scene();
        let patch: lumina_core::ScenePatch = serde_json::from_value(serde_json::json!({
            "patches": [{
                "op": "add_object",
                "id": "c2",
                "type": "Circle",
                "properties": { "cx": 10.0, "cy": 10.0, "radius": 5.0, "fill": "#00FF00" }
            }]
        }))
        .unwrap();
        lumina_core::apply_patch(&mut scene, &patch).expect("patch applies");
        assert!(scene.objects.contains_key("c2"));
        let validation = validate_scene_data(&scene);
        assert!(validation.valid, "patched scene should validate clean");
    }

    #[test]
    fn test_valid_scene_passes() {
        let scene = minimal_scene();
        let result = validate_scene_data(&scene);
        assert!(result.valid, "Expected valid scene: {:?}", result.errors);
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
        scene.objects.insert(
            "group_a".into(),
            Object::Group(GroupProps {
                children: vec!["group_b".into()],
                x: 0.0,
                y: 0.0,
                z_index: 0,
                scale: 1.0,
                rotation: 0.0,
                opacity: 1.0,
            }),
        );
        scene.objects.insert(
            "group_b".into(),
            Object::Group(GroupProps {
                children: vec!["group_a".into()],
                x: 0.0,
                y: 0.0,
                z_index: 0,
                scale: 1.0,
                rotation: 0.0,
                opacity: 1.0,
            }),
        );
        let result = validate_scene_data(&scene);
        assert!(!result.valid);
        assert!(result
            .errors
            .iter()
            .any(|e| e.code == "CIRCULAR_GROUP_REFERENCE"));
    }

    #[test]
    fn test_keyframe_beyond_duration_is_warning() {
        let mut scene = minimal_scene();
        scene.timeline.push(TimelineEntry {
            time: 999.0,
            object: "circle".into(),
            state: serde_json::json!({"opacity": 1.0}),
            easing: "linear".into(),
            easing_params: None,
        });
        let result = validate_scene_data(&scene);
        assert!(result.valid, "Beyond-duration keyframe should be a warning");
        assert!(result
            .warnings
            .iter()
            .any(|w| w.code == "KEYFRAME_BEYOND_DURATION"));
    }

    #[test]
    fn test_zero_canvas_size_is_error() {
        let mut scene = minimal_scene();
        scene.canvas.width = 0;
        let result = validate_scene_data(&scene);
        assert!(!result.valid);
        assert!(result
            .errors
            .iter()
            .any(|e| e.code == "INVALID_CANVAS_SIZE"));
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
        assert!(result
            .warnings
            .iter()
            .any(|w| w.code == "DUPLICATE_KEYFRAME"));
    }

    #[test]
    fn test_schema_endpoint_produces_valid_json_schema() {
        let schema = schemars::schema_for!(Scene);
        let value = serde_json::to_value(&schema).expect("schema must serialise");
        assert!(
            value.get("$schema").is_some() || value.get("title").is_some(),
            "Schema should have $schema or title field"
        );
    }

    #[test]
    fn test_patch_applies_correctly() {
        use json_patch::patch;
        use serde_json::json;

        let mut scene_val = serde_json::to_value(minimal_scene()).unwrap();
        let ops: json_patch::Patch = serde_json::from_value(
            json!([{"op": "replace", "path": "/meta/title", "value": "Patched"}]),
        )
        .unwrap();
        patch(&mut scene_val, &ops).unwrap();
        assert_eq!(scene_val["meta"]["title"], "Patched");
    }

    #[test]
    fn test_object_registry_covers_all_types() {
        let registry = object_registry();
        let obj = registry
            .as_object()
            .expect("registry must be a JSON object");
        // Every object type should be present with a non-empty required list.
        for ty in [
            "Circle",
            "Rectangle",
            "Polygon",
            "Path",
            "Line",
            "Arrow",
            "Text",
            "LaTeX",
            "MathML",
            "Image",
            "SVG",
            "Group",
            "NumberLine",
            "Axes",
            "Plot",
            "BezierCurve",
            "Particles",
        ] {
            let entry = obj.get(ty).unwrap_or_else(|| panic!("missing type {ty}"));
            assert!(
                entry["required"].as_array().is_some_and(|a| !a.is_empty()),
                "{ty} should declare required properties"
            );
            assert!(
                entry["optional"].is_array(),
                "{ty} should declare optional properties"
            );
        }
    }
}
