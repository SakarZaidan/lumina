//! Python bindings for the Lumina animation engine.
//!
//! Exposes four functions to Python:
//!   - `luminafx.validate(scene_dict) -> dict`
//!   - `luminafx.fix(scene_dict) -> dict`
//!   - `luminafx.render(scene_dict, output_path, format="mp4")`
//!   - `luminafx.schema() -> dict`

// The engine has never contained `unsafe`, and the metric tracking that was a
// `grep` over the source — which by v0.4.0 was returning a false positive from
// the word appearing in a comment. `forbid` makes it a compile error instead:
// it cannot be silenced by an `allow` further down, so a future `unsafe` block
// has to be argued for by removing this line, in a diff a reviewer will see.
#![forbid(unsafe_code)]
use luminafx_core::validation::{validate_scene_json, ValidationError, ValidationResponse};
use luminafx_export::Exporter;
use luminafx_renderer::skia_backend::SkiaRenderer;
use luminafx_renderer::Renderer;
use luminafx_schema::Scene;
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;

/// Validate a scene dict. Returns a dict `{valid, errors, warnings}` with the
/// same structure as the server's `/validate` endpoint, including
/// `fix_suggestion` strings ready to feed back into an LLM correction loop.
///
/// Validated from the dict itself rather than a parsed scene. Parsing drops a
/// property it does not know, so `"opacty": 0.5` used to come back valid here
/// while `lumina-cli validate` and the server rejected it (RFC-0002).
#[pyfunction]
fn validate(py: Python<'_>, scene: &Bound<'_, PyAny>) -> PyResult<PyObject> {
    let response = match raw_scene(scene) {
        Ok(raw) => validate_scene_json(&raw),
        Err(response) => response,
    };
    Ok(pythonize::pythonize(py, &response)?.into())
}

/// Repair every mistake in a scene dict that has exactly one sensible fix — a
/// misspelled property, object id, object type, asset id or easing name — and
/// re-validate until nothing more can be fixed that way.
///
/// Returns `{scene, applied, remaining}`: the repaired scene, each fix applied,
/// and the validation of what is left, which needs a decision.
#[pyfunction]
fn fix(py: Python<'_>, scene: &Bound<'_, PyAny>) -> PyResult<PyObject> {
    let raw = raw_scene(scene)
        .map_err(|response| PyValueError::new_err(describe_errors("not a scene", &response)))?;
    let report = luminafx_core::fix::fix_scene(&raw, luminafx_core::fix::DEFAULT_MAX_ROUNDS);
    Ok(pythonize::pythonize(py, &report)?.into())
}

/// A Python value as JSON, or the validation response that says why it is not.
fn raw_scene(scene: &Bound<'_, PyAny>) -> Result<serde_json::Value, ValidationResponse> {
    pythonize::depythonize::<serde_json::Value>(scene).map_err(|e| ValidationResponse {
        valid: false,
        errors: vec![ValidationError {
            code: "PARSE_ERROR".to_string(),
            path: "$".to_string(),
            message: format!("the scene is not a JSON-like value: {e}"),
            fix_suggestion: "Pass a dict of strings, numbers, lists and dicts \
                             (see luminafx.schema())."
                .to_string(),
            fix_patch: None,
        }],
        warnings: Vec::new(),
    })
}

/// One line per error, for an exception message.
fn describe_errors(what: &str, response: &ValidationResponse) -> String {
    let mut text = format!("{what}:");
    for e in &response.errors {
        text.push_str(&format!("\n  {} at {}: {}", e.code, e.path, e.message));
    }
    text
}

/// Render a scene dict to a file. `format` is "mp4" (default) or "png" (a frame
/// sequence written into the output directory). Fonts and images declared in
/// `assets` are loaded from their on-disk paths.
#[pyfunction]
#[pyo3(signature = (scene, output_path, format="mp4"))]
fn render(scene: &Bound<'_, PyAny>, output_path: String, format: &str) -> PyResult<()> {
    // Validated before anything is decoded or drawn, and from the dict, so
    // render refuses exactly what `validate` reports — as the CLI, the server
    // and the MCP tool do. It used to render any dict that parsed.
    let raw = raw_scene(scene)
        .map_err(|response| PyValueError::new_err(describe_errors("not a scene", &response)))?;
    let validation = validate_scene_json(&raw);
    if !validation.valid {
        return Err(PyValueError::new_err(describe_errors(
            "the scene does not validate, so it was not rendered",
            &validation,
        )));
    }
    let scene: Scene = serde_json::from_value(raw)
        .map_err(|e| PyValueError::new_err(format!("Invalid scene: {e}")))?;

    let mut renderer = SkiaRenderer::new();
    for font in &scene.assets.fonts {
        if let Ok(data) = std::fs::read(&font.path) {
            let _ = renderer.load_font(&font.id, &data);
        }
    }
    for img in &scene.assets.images {
        if let Ok(data) = std::fs::read(&img.path) {
            let _ = renderer.load_image(&img.id, &data);
        }
    }

    let mut exporter = Exporter::new(renderer);
    let path = std::path::Path::new(&output_path);
    let result = match format {
        "png" => exporter.export_png_sequence(&scene, path),
        "mp4" => exporter.export_mp4(&scene, path),
        other => {
            return Err(PyValueError::new_err(format!(
                "Unknown format '{other}'. Use 'mp4' or 'png'."
            )))
        }
    };
    result.map_err(|e| PyRuntimeError::new_err(format!("Render failed: {e}")))
}

/// Return the LSF JSON Schema as a Python dict.
#[pyfunction]
fn schema(py: Python<'_>) -> PyResult<PyObject> {
    let schema = schemars::schema_for!(Scene);
    Ok(pythonize::pythonize(py, &schema)?.into())
}

#[pymodule]
fn luminafx(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(validate, m)?)?;
    m.add_function(wrap_pyfunction!(fix, m)?)?;
    m.add_function(wrap_pyfunction!(render, m)?)?;
    m.add_function(wrap_pyfunction!(schema, m)?)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
