//! Animation runtime for the Lumina engine.
//!
//! Consumes the data model from `lumina-schema` and provides all engine
//! logic:
//!
//! - [`scene::SceneGraph`] — object map with root/child resolution through
//!   groups.
//! - [`timeline::Timeline`] — per-object, per-property keyframe tracks;
//!   evaluate the full scene state at any time with `get_state_at`, and the
//!   camera with `get_camera_at`. Evaluation is deterministic and scrub-safe.
//! - [`easing`] — 28 named easing functions (Manim-compatible and CSS
//!   aliases) plus parameterized `cubic_bezier` and overshoot-free monotone
//!   `spline`.
//! - [`interpolator`] — JSON-value interpolation: numeric lerp, element-wise
//!   arrays (path morphing), and hex colors blended in CIELAB space.
//! - [`events`] — [`EventBus`], playback state, and interactive actions.
//! - [`scene_patch`] — semantic patch operations (`add_object`,
//!   `add_keyframe`, …) for programmatic scene editing.
//! - [`validation`] — semantic scene validation with machine-actionable
//!   `fix_suggestion`s (shared by the server, CLI and SDKs).
//!
//! Rendering lives downstream in `lumina-renderer`; this crate never touches
//! pixels.

// The engine has never contained `unsafe`, and the metric tracking that was a
// `grep` over the source — which by v0.4.0 was returning a false positive from
// the word appearing in a comment. `forbid` makes it a compile error instead:
// it cannot be silenced by an `allow` further down, so a future `unsafe` block
// has to be argued for by removing this line, in a diff a reviewer will see.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// Named easing functions plus the canonical name registry.
pub mod easing;
/// Interactive event dispatch and playback state.
pub mod events;
/// JSON-value interpolation (numbers, arrays, LAB-space colors).
pub mod interpolator;
/// Scene graph: object map with root/child resolution.
pub mod scene;
pub mod scene_patch;
/// "Did you mean …?" for unknown identifiers, shared by every reference check.
pub mod suggest;
/// Keyframe tracks and scene-state evaluation at any time.
pub mod timeline;
pub mod validation;

#[cfg(test)]
mod easing_proptests;
#[cfg(test)]
mod easing_tests;
#[cfg(test)]
mod events_tests;
#[cfg(test)]
mod interp_proptests;
#[cfg(test)]
mod interp_tests;
#[cfg(test)]
mod scene_patch_tests;
#[cfg(test)]
mod stress_tests;
#[cfg(test)]
mod timeline_tests;
#[cfg(test)]
mod validation_tests;

pub use events::{EmittedEvent, Event, EventBus, EventOutcome, PlaybackState};
pub use scene::SceneGraph;
pub use scene_patch::{apply_patch, PatchError, PatchOp, ScenePatch};
pub use timeline::Timeline;

/// Every LSF object type with its required and optional properties.
///
/// Lives here rather than in the HTTP server because it is now answered by two
/// front ends — `GET /objects` and the MCP `lumina_objects` tool — and a
/// registry maintained in two places is a registry that disagrees with itself.
/// That is the same mistake TD-02 recorded for the path and colour parsers.
///
/// Deliberately a hand-written summary rather than generated from the schema:
/// it is what an agent reads *first*, and the point is that it is small enough
/// to read. The full `schemars` output is available separately for when the
/// exact shape matters.
#[must_use]
pub fn object_registry() -> serde_json::Value {
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
