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
/// Keyframe tracks and scene-state evaluation at any time.
pub mod timeline;
pub mod validation;

#[cfg(test)]
mod easing_tests;
#[cfg(test)]
mod events_tests;
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
