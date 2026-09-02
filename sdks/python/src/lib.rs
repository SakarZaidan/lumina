//! Python bindings for the Lumina animation engine.
//!
//! Exposes three functions to Python:
//!   - `lumina.validate(scene_dict) -> dict`
//!   - `lumina.render(scene_dict, output_path, format="mp4")`
//!   - `lumina.schema() -> dict`

// The engine has never contained `unsafe`, and the metric tracking that was a
// `grep` over the source — which by v0.4.0 was returning a false positive from
// the word appearing in a comment. `forbid` makes it a compile error instead:
// it cannot be silenced by an `allow` further down, so a future `unsafe` block
// has to be argued for by removing this line, in a diff a reviewer will see.
#![forbid(unsafe_code)]
use lumina_export::Exporter;
use lumina_renderer::skia_backend::SkiaRenderer;
use lumina_renderer::Renderer;
use lumina_schema::Scene;
use lumina_server::validate_scene_data;
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;

/// Validate a scene dict. Returns a dict `{valid, errors, warnings}` with the
/// same structure as the server's `/validate` endpoint, including
/// `fix_suggestion` strings ready to feed back into an LLM correction loop.
#[pyfunction]
fn validate(py: Python<'_>, scene: &Bound<'_, PyAny>) -> PyResult<PyObject> {
    match pythonize::depythonize::<Scene>(scene) {
        Ok(parsed) => {
            let resp = validate_scene_data(&parsed);
            Ok(pythonize::pythonize(py, &resp)?.into())
        }
        Err(e) => {
            // Surface schema-level parse failures in the same structured shape.
            let resp = serde_json::json!({
                "valid": false,
                "errors": [{
                    "code": "PARSE_ERROR",
                    "path": "$",
                    "message": e.to_string(),
                    "fix_suggestion": "Ensure the scene matches the LSF schema (see lumina.schema())."
                }],
                "warnings": []
            });
            Ok(pythonize::pythonize(py, &resp)?.into())
        }
    }
}

/// Render a scene dict to a file. `format` is "mp4" (default) or "png" (a frame
/// sequence written into the output directory). Fonts and images declared in
/// `assets` are loaded from their on-disk paths.
#[pyfunction]
#[pyo3(signature = (scene, output_path, format="mp4"))]
fn render(scene: &Bound<'_, PyAny>, output_path: String, format: &str) -> PyResult<()> {
    let scene: Scene = pythonize::depythonize(scene)
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
fn lumina(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(validate, m)?)?;
    m.add_function(wrap_pyfunction!(render, m)?)?;
    m.add_function(wrap_pyfunction!(schema, m)?)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
