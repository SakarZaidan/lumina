//! The tools an agent can call, and what each one answers.
//!
//! Every tool answers in the shape the HTTP server's error envelope
//! established: a `code`, a `message`, and — wherever the answer is knowable —
//! a `fix_suggestion`. That consistency is the point of this crate. An agent
//! driving Lumina should not have to learn two error vocabularies depending on
//! whether it went through MCP or HTTP.

use serde_json::{json, Value};

use luminafx_core::scene_patch::ScenePatch;
use luminafx_core::validation::validate_scene_data;
use luminafx_schema::Scene;

/// Every tool this server exposes, as MCP tool descriptors.
///
/// The descriptions are written for a model rather than a human: they say what
/// the tool is *for* and when to reach for it, because that is what a model
/// uses to choose. "Validates a scene" would be accurate and useless.
#[must_use]
pub fn descriptors() -> Value {
    json!([
        {
            "name": "lumina_schema",
            "description":
                "Get the JSON Schema for the Lumina Scene Format (LSF). Call this first when \
                 writing a scene from scratch, or when a validation error mentions a field you \
                 do not recognise. Pass `objects` to get only the types you need — the full \
                 schema is large and most tasks touch two or three object types.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "objects": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description":
                            "Object type names to restrict the schema to, e.g. [\"Circle\", \
                             \"Text\"]. Omit for the whole schema."
                    }
                }
            }
        },
        {
            "name": "lumina_objects",
            "description":
                "List every LSF object type with its required and optional properties. Cheaper \
                 than the full schema and usually enough to write a scene. Start here.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "lumina_validate",
            "description":
                "Check a scene without rendering it. Returns structured errors, each with a \
                 `code`, a JSON `path`, and a `fix_suggestion` you can act on directly. Always \
                 validate before rendering: rendering is seconds of CPU and validation is \
                 microseconds.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "scene": { "type": "object", "description": "The LSF scene document." }
                },
                "required": ["scene"]
            }
        },
        {
            "name": "lumina_patch",
            "description":
                "Apply a semantic patch to a scene and re-validate in one step. Use this to \
                 repair a scene rather than resending the whole document — it is smaller, and \
                 the response tells you whether the repair worked.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "scene": { "type": "object", "description": "The scene to modify." },
                    "patch": { "type": "object", "description": "A semantic patch operation." }
                },
                "required": ["scene", "patch"]
            }
        },
        {
            "name": "lumina_render",
            "description":
                "Render a scene to a file on disk and return the path. Validates first and \
                 refuses to render an invalid scene, so a failure here is a real rendering \
                 problem rather than a malformed document. Formats: png (frame sequence), mp4, \
                 webm, gif, exr.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "scene": { "type": "object", "description": "The LSF scene document." },
                    "output": {
                        "type": "string",
                        "description": "Path to write. Must be inside the server's asset root."
                    },
                    "format": {
                        "type": "string",
                        "enum": ["png", "mp4", "webm", "gif", "exr"],
                        "description": "Output format; defaults to mp4."
                    }
                },
                "required": ["scene", "output"]
            }
        }
    ])
}

/// A tool result an agent can read, in the envelope the HTTP surface uses.
pub struct ToolResult {
    /// The payload.
    pub value: Value,
    /// Whether this represents a failure. MCP carries errors in the result
    /// rather than as protocol errors, so a model sees them as content it can
    /// reason about instead of a transport failure it cannot.
    pub is_error: bool,
}

impl ToolResult {
    fn ok(value: Value) -> Self {
        Self {
            value,
            is_error: false,
        }
    }

    fn err(code: &str, message: impl Into<String>, fix: Option<&str>) -> Self {
        let mut v = json!({ "code": code, "message": message.into() });
        if let Some(fix) = fix {
            v["fix_suggestion"] = json!(fix);
        }
        Self {
            value: v,
            is_error: true,
        }
    }
}

/// Parse the `scene` argument, or explain why it is not a scene.
fn scene_arg(args: &Value) -> Result<Scene, ToolResult> {
    let Some(raw) = args.get("scene") else {
        return Err(ToolResult::err(
            "MISSING_ARGUMENT",
            "this tool needs a `scene` argument",
            Some("Pass the LSF document as `scene`. Call lumina_objects for its shape."),
        ));
    };
    serde_json::from_value(raw.clone()).map_err(|e| {
        ToolResult::err(
            "SCHEMA_MISMATCH",
            format!("the value is valid JSON but not a scene: {e}"),
            Some("Call lumina_schema for the exact shape, or lumina_objects for a summary."),
        )
    })
}

/// Dispatch a tool call.
pub fn call(name: &str, args: &Value) -> ToolResult {
    match name {
        "lumina_objects" => ToolResult::ok(luminafx_core::object_registry()),
        "lumina_schema" => {
            let schema =
                serde_json::to_value(schemars::schema_for!(Scene)).unwrap_or_else(|_| json!({}));
            let Some(wanted) = args.get("objects").and_then(Value::as_array) else {
                return ToolResult::ok(schema);
            };
            // A scoped schema is not a smaller schema with fields dropped: the
            // definitions a model needs are exactly the ones it will be asked
            // to fill in, and pruning the rest is what makes the difference
            // between a 40 kB paste and a 3 kB one (AAA-AI-05).
            let names: Vec<String> = wanted
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect();
            ToolResult::ok(scope_schema(&schema, &names))
        }
        "lumina_validate" => match scene_arg(args) {
            Ok(scene) => ToolResult::ok(
                serde_json::to_value(validate_scene_data(&scene)).unwrap_or_else(|_| json!({})),
            ),
            Err(e) => e,
        },
        "lumina_patch" => {
            let mut scene = match scene_arg(args) {
                Ok(s) => s,
                Err(e) => return e,
            };
            let Some(raw) = args.get("patch") else {
                return ToolResult::err(
                    "MISSING_ARGUMENT",
                    "this tool needs a `patch` argument",
                    Some("A patch is an object naming an operation and its target."),
                );
            };
            let patch: ScenePatch = match serde_json::from_value(raw.clone()) {
                Ok(p) => p,
                Err(e) => {
                    return ToolResult::err(
                        "MALFORMED_PATCH",
                        format!("that is not a scene patch: {e}"),
                        Some(
                            "A patch names an operation and its target, e.g. \
                              {\"op\": \"set_property\", \"object_id\": \"c\", \
                              \"property\": \"radius\", \"value\": 40}.",
                        ),
                    )
                }
            };
            if let Err(e) = luminafx_core::apply_patch(&mut scene, &patch) {
                return ToolResult::err(
                    "PATCH_FAILED",
                    format!("the patch could not be applied: {e}"),
                    Some("Call lumina_objects for the properties each object type accepts."),
                );
            }
            let validation = validate_scene_data(&scene);
            ToolResult::ok(json!({
                "scene": serde_json::to_value(&scene).unwrap_or_else(|_| json!({})),
                "validation": serde_json::to_value(&validation).unwrap_or_else(|_| json!({})),
            }))
        }
        "lumina_render" => render(args),
        _ => ToolResult::err(
            "UNKNOWN_TOOL",
            format!("no tool named `{name}`"),
            Some("Call tools/list for the available tools."),
        ),
    }
}

/// Keep only the definitions for `wanted`, plus everything they reference.
///
/// Transitive on purpose: `Circle` alone is useless without `Shadow`, and a
/// model handed a schema with a dangling `$ref` will either invent the missing
/// type or refuse. Anything unrecognised is left in rather than dropped —
/// erring toward a larger schema is a cost, erring toward a broken one is a
/// failure.
fn scope_schema(schema: &Value, wanted: &[String]) -> Value {
    let mut out = schema.clone();
    let Some(defs) = schema.get("definitions").or_else(|| schema.get("$defs")) else {
        return out;
    };
    let Some(defs) = defs.as_object() else {
        return out;
    };

    let mut keep: Vec<String> = wanted.to_vec();
    let mut i = 0;
    while i < keep.len() {
        let name = keep[i].clone();
        i += 1;
        let Some(def) = defs.get(&name) else { continue };
        for r in refs_in(def) {
            if !keep.contains(&r) {
                keep.push(r);
            }
        }
    }

    let pruned: serde_json::Map<String, Value> = defs
        .iter()
        .filter(|(k, _)| keep.contains(k))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    if let Some(obj) = out.as_object_mut() {
        let key = if obj.contains_key("$defs") {
            "$defs"
        } else {
            "definitions"
        };
        obj.insert(key.to_string(), Value::Object(pruned));
    }
    out
}

/// Every definition name a `$ref` in `value` points at, at any depth.
fn refs_in(value: &Value) -> Vec<String> {
    let mut found = Vec::new();
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                if k == "$ref" {
                    if let Some(name) = v.as_str().and_then(|r| r.rsplit('/').next()) {
                        found.push(name.to_string());
                    }
                } else {
                    found.extend(refs_in(v));
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                found.extend(refs_in(item));
            }
        }
        _ => {}
    }
    found
}

/// Render a scene to a file and answer with the path.
///
/// Validates first and refuses an invalid scene, so a failure from this tool
/// is a real rendering problem rather than a malformed document — which means
/// an agent can act on it differently. Rendering an invalid scene would burn
/// seconds of CPU to produce an error the validator gives back instantly.
fn render(args: &Value) -> ToolResult {
    let scene = match scene_arg(args) {
        Ok(s) => s,
        Err(e) => return e,
    };
    let Some(output) = args.get("output").and_then(Value::as_str) else {
        return ToolResult::err(
            "MISSING_ARGUMENT",
            "this tool needs an `output` path",
            Some("Pass `output`, e.g. \"out/scene.mp4\"."),
        );
    };

    let validation = validate_scene_data(&scene);
    if !validation.valid {
        return ToolResult {
            value: json!({
                "code": "SCENE_INVALID",
                "message": "the scene does not validate, so it was not rendered",
                "fix_suggestion":
                    "Each error below carries a `path` and a `fix_suggestion`. Apply them with \
                     lumina_patch and validate again.",
                "errors": serde_json::to_value(&validation.errors).unwrap_or_else(|_| json!([])),
            }),
            is_error: true,
        };
    }

    let Some(path) = resolve_output(output) else {
        return ToolResult::err(
            "OUTPUT_OUTSIDE_ROOT",
            format!("`{output}` is outside the directory this server may write to"),
            Some(
                "Set LUMINA_ASSET_ROOT to the directory you want written, or pass a path \
                  inside the current one.",
            ),
        );
    };

    let format = args.get("format").and_then(Value::as_str).unwrap_or("mp4");

    let mut exporter =
        luminafx_export::Exporter::new(luminafx_renderer::skia_backend::SkiaRenderer::new());
    let result = match format {
        "png" => exporter.export_png_sequence(&scene, &path),
        "exr" => exporter.export_exr_sequence(&scene, &path),
        "webm" => exporter.export_webm(&scene, &path),
        "gif" => exporter.export_gif(&scene, &path),
        "mp4" => exporter.export_mp4(&scene, &path),
        other => {
            return ToolResult::err(
                "UNKNOWN_FORMAT",
                format!("no format `{other}`"),
                Some("One of: png, exr, mp4, webm, gif."),
            )
        }
    };

    match result {
        Ok(()) => ToolResult::ok(json!({
            "output": path.display().to_string(),
            "format": format,
            "frames": (scene.canvas.duration * scene.canvas.fps as f32).ceil() as u32,
        })),
        Err(e) => ToolResult::err(
            "RENDER_FAILED",
            format!("{e}"),
            Some("Video formats need ffmpeg on PATH. `png` and `exr` do not."),
        ),
    }
}

/// Confine an output path to the directory this server may write to.
///
/// The same reasoning as the HTTP server's asset root, in the other direction:
/// this tool is invoked by a model, on a developer's machine, with that
/// developer's permissions. "Render to `../../.ssh/authorized_keys`" must not
/// be a thing it can be talked into.
///
/// The parent is canonicalised rather than the path itself, because the output
/// does not exist yet — canonicalising it would always fail.
fn resolve_output(requested: &str) -> Option<std::path::PathBuf> {
    let root = std::env::var_os("LUMINA_ASSET_ROOT")
        .map_or_else(|| std::path::PathBuf::from("."), std::path::PathBuf::from)
        .canonicalize()
        .ok()?;

    let candidate = if std::path::Path::new(requested).is_absolute() {
        std::path::PathBuf::from(requested)
    } else {
        root.join(requested)
    };

    let parent = candidate.parent()?;
    // Create it first: a directory that does not exist cannot be canonicalised,
    // and refusing to render into a new subdirectory would be surprising.
    std::fs::create_dir_all(parent).ok()?;
    let parent = parent.canonicalize().ok()?;
    if !parent.starts_with(&root) {
        return None;
    }
    Some(parent.join(candidate.file_name()?))
}
