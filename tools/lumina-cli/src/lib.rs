//! Command implementations for the Lumina CLI.
//!
//! The logic lives here rather than in `main.rs` for one reason: a 470-line
//! `main` cannot be tested, and coverage said so — the binary measured **0%**
//! while the rest of the workspace sat above 80% (TD-10). `main` now parses
//! argv and calls into this module; everything that decides anything is here,
//! where a test can reach it.
//!
//! # The command surface
//!
//! Subcommands (`render`, `validate`, `schema`, `objects`, `new`, `inspect`)
//! are the surface going forward. The original flat form —
//! `lumina-cli --scene x.lsf --output y --format mp4` — still works and is
//! still what every example and CI job uses, because breaking those to gain a
//! nicer noun-verb shape would be paying a real cost for a stylistic one. It
//! is deprecated in the help text rather than removed.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::path::{Path, PathBuf};

use luminafx_core::validation::{validate_scene_data, ValidationResponse};
use luminafx_schema::Scene;

/// Read and parse a scene file.
///
/// # Errors
///
/// Returns an error if the file cannot be read or is not valid LSF. Parse
/// errors carry the line and column, because a scene is hand-written or
/// model-written JSON and "expected `,`" without a position is not actionable.
pub fn load_scene(path: &Path) -> anyhow::Result<Scene> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", path.display()))?;
    serde_json::from_str(&text)
        .map_err(|e| anyhow::anyhow!("{} is not a valid scene: {e}", path.display()))
}

/// How a validation result should be rendered to a terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Report {
    /// One line per finding, for a person.
    Human,
    /// The whole `ValidationResponse` as JSON, for a script or an agent.
    Json,
}

/// Format a validation result for display.
///
/// Returns the text to print and whether the scene passed. Separated from
/// printing so a test can read the output without capturing stdout.
#[must_use]
pub fn format_validation(result: &ValidationResponse, style: Report) -> (String, bool) {
    if style == Report::Json {
        let text = serde_json::to_string_pretty(result)
            .unwrap_or_else(|e| format!(r#"{{"error":"{e}"}}"#));
        return (text, result.valid);
    }

    let mut out = String::new();
    for w in &result.warnings {
        out.push_str(&format!(
            "warning: {} at {}\n  {}\n",
            w.code, w.path, w.message
        ));
    }
    for e in &result.errors {
        out.push_str(&format!(
            "error: {} at {}\n  {}\n  fix: {}\n",
            e.code, e.path, e.message, e.fix_suggestion
        ));
    }
    if result.valid {
        let n = result.warnings.len();
        out.push_str(&match n {
            0 => "ok\n".to_string(),
            1 => "ok, 1 warning\n".to_string(),
            _ => format!("ok, {n} warnings\n"),
        });
    } else {
        out.push_str(&format!(
            "{} error(s), {} warning(s)\n",
            result.errors.len(),
            result.warnings.len()
        ));
    }
    (out, result.valid)
}

/// Validate a scene file and report.
///
/// # Errors
///
/// Returns an error if the file cannot be read or parsed. A scene that parses
/// but fails validation is *not* an error here — the caller decides the exit
/// code, because `render` and `validate` want different things from the same
/// answer.
pub fn validate(path: &Path, style: Report) -> anyhow::Result<(String, bool)> {
    let scene = load_scene(path)?;
    Ok(format_validation(&validate_scene_data(&scene), style))
}

/// The JSON Schema for the scene format.
#[must_use]
pub fn schema_json() -> String {
    serde_json::to_string_pretty(&schemars::schema_for!(Scene))
        .unwrap_or_else(|e| format!(r#"{{"error":"{e}"}}"#))
}

/// The object-type registry: every type with its required and optional
/// properties.
#[must_use]
pub fn objects_json() -> String {
    serde_json::to_string_pretty(&luminafx_core::object_registry())
        .unwrap_or_else(|e| format!(r#"{{"error":"{e}"}}"#))
}

/// A minimal scene that renders something, as a starting point.
///
/// Deliberately not empty. A template that produces a black rectangle teaches
/// nothing and gives no signal that the toolchain works; this one animates, so
/// rendering it confirms the whole pipeline end to end.
#[must_use]
pub fn scene_template(title: &str) -> String {
    let scene = serde_json::json!({
        "version": "1.0",
        "meta": {
            "title": title,
            "author": "",
            "created_at": "2026-01-01T00:00:00Z"
        },
        "canvas": {
            "width": 1920, "height": 1080, "fps": 60, "duration": 3.0,
            "background": "#0B0D17"
        },
        "objects": {
            "dot": {
                "type": "Circle",
                "properties": {
                    "cx": 480.0, "cy": 540.0, "radius": 80.0,
                    "fill": "#4DABF7", "z_index": 1
                }
            }
        },
        "timeline": [
            { "time": 0.0, "object": "dot", "state": { "cx": 480.0 }, "easing": "linear" },
            { "time": 3.0, "object": "dot", "state": { "cx": 1440.0 },
              "easing": "ease_in_out_cubic" }
        ]
    });
    serde_json::to_string_pretty(&scene).unwrap_or_default()
}

/// Write a starter scene to `path`, refusing to clobber an existing file.
///
/// # Errors
///
/// Returns an error if the file already exists or cannot be written.
pub fn new_scene(path: &Path) -> anyhow::Result<()> {
    if path.exists() {
        anyhow::bail!(
            "{} already exists — refusing to overwrite it",
            path.display()
        );
    }
    let title = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("untitled");
    std::fs::write(path, scene_template(title))
        .map_err(|e| anyhow::anyhow!("cannot write {}: {e}", path.display()))?;
    Ok(())
}

/// Plot an easing curve as ASCII, `width` by `height` characters.
///
/// For choosing a curve without rendering a video to see it. Easings are the
/// one part of the format whose effect is impossible to read from its name —
/// `ease_out_back` overshoots, and nothing about the name says by how much.
///
/// # Errors
///
/// Returns an error naming the nearest match if the easing is unknown.
pub fn plot_easing(name: &str, width: usize, height: usize) -> anyhow::Result<String> {
    if !luminafx_core::easing::EASING_NAMES.contains(&name) {
        match luminafx_core::easing::suggest_easing(name) {
            Some(s) => anyhow::bail!("unknown easing `{name}` — did you mean `{s}`?"),
            None => anyhow::bail!(
                "unknown easing `{name}`. `lumina-cli inspect --list` shows all {}",
                luminafx_core::easing::EASING_NAMES.len()
            ),
        }
    }

    // Sample first, then scale to what was actually produced: several easings
    // leave [0, 1] on purpose — `ease_out_back` overshoots and `ease_in_back`
    // undershoots — and a plot clamped to the unit square would hide the one
    // feature those curves are chosen for.
    let samples: Vec<(f32, f32)> = (0..width)
        .map(|i| {
            let t = i as f32 / (width - 1).max(1) as f32;
            (t, luminafx_core::easing::eval_easing(name, None, t))
        })
        .collect();

    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for (_, v) in &samples {
        lo = lo.min(*v);
        hi = hi.max(*v);
    }
    let span = (hi - lo).max(f32::EPSILON);

    let mut grid = vec![vec![' '; width]; height];
    for (i, (_, v)) in samples.iter().enumerate() {
        let norm = (v - lo) / span;
        let row = height - 1 - ((norm * (height - 1) as f32).round() as usize).min(height - 1);
        grid[row][i] = '#';
    }

    let mut out = format!("{name}  [{lo:.2} … {hi:.2}]\n");
    for row in grid {
        out.push_str("  |");
        out.extend(row);
        out.push('\n');
    }
    out.push_str("  +");
    out.push_str(&"-".repeat(width));
    out.push('\n');
    out.push_str("   0");
    out.push_str(&" ".repeat(width.saturating_sub(2)));
    out.push_str("1\n");
    Ok(out)
}

/// Every registered easing name, one per line.
#[must_use]
pub fn easing_list() -> String {
    let mut names: Vec<&str> = luminafx_core::easing::EASING_NAMES.to_vec();
    names.sort_unstable();
    let mut out = format!("{} easings:\n", names.len());
    for n in names {
        out.push_str("  ");
        out.push_str(n);
        out.push('\n');
    }
    out
}

/// Resolve the output path for a render, defaulting the extension to match
/// the format.
///
/// `--format mp4 -o out` should write `out.mp4`, not a file called `out` that
/// no player will open on a double-click. Frame-sequence formats are
/// directories and keep the bare name.
#[must_use]
pub fn output_path(requested: &Path, format: &str) -> PathBuf {
    if matches!(format, "png" | "exr") || requested.extension().is_some() {
        return requested.to_path_buf();
    }
    let ext = match format {
        "webm" | "webm-alpha" => "webm",
        "mov" | "prores" => "mov",
        "gif" => "gif",
        _ => "mp4",
    };
    requested.with_extension(ext)
}

#[cfg(test)]
mod tests;
