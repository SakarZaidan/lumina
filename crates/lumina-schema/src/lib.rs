//! Scene data model for the Lumina animation engine.
//!
//! This is the leaf crate of the workspace: pure serde/schemars types with no
//! logic. It defines the Lumina Scene Format (LSF) — [`Scene`], the 17-variant
//! [`Object`] enum with per-type property structs, [`Paint`] (solid colors and
//! gradients), timeline and event entries, and the camera model — plus every
//! default value used by the engine.
//!
//! All types derive [`schemars::JsonSchema`], so the full JSON Schema of the
//! scene format can be generated for validation and AI/authoring tooling
//! (see the `/schema` endpoint in `lumina-server`).
//!
//! Layering: `lumina-schema` → `lumina-core` → `lumina-renderer` →
//! `lumina-export`. Runtime behavior lives upstream; this crate is data only.

// The engine has never contained `unsafe`, and the metric tracking that was a
// `grep` over the source — which by v0.4.0 was returning a false positive from
// the word appearing in a comment. `forbid` makes it a compile error instead:
// it cannot be silenced by an `allow` further down, so a future `unsafe` block
// has to be argued for by removing this line, in a diff a reviewer will see.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A complete Lumina scene document (LSF): metadata, canvas, assets,
/// objects, timeline, events, and optional camera.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Scene {
    /// LSF format version string.
    pub version: String,
    /// Document metadata.
    pub meta: Meta,
    /// Canvas definition.
    pub canvas: Canvas,
    /// Fonts and images to load.
    #[serde(default)]
    pub assets: Assets,
    /// All objects, keyed by unique id.
    pub objects: HashMap<String, Object>,
    /// Keyframes, in any order.
    pub timeline: Vec<TimelineEntry>,
    /// Interactive event bindings.
    #[serde(default)]
    pub events: Vec<EventEntry>,
    /// Optional camera with its own timeline.
    #[serde(default)]
    pub camera: Option<Camera>,
}

/// Document metadata (informational only).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Meta {
    /// Human-readable title.
    pub title: String,
    /// Author name.
    pub author: String,
    /// Creation timestamp (ISO-8601).
    pub created_at: String,
}

/// Output canvas: pixel dimensions, frame rate, duration, background.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Canvas {
    /// Canvas width in pixels.
    pub width: u32,
    /// Canvas height in pixels.
    pub height: u32,
    /// Frames per second.
    pub fps: u32,
    /// Duration in seconds.
    pub duration: f32,
    /// Background color as a hex string.
    pub background: String,
}

/// Font and image assets referenced by objects.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Assets {
    /// Font files to load.
    #[serde(default)]
    pub fonts: Vec<FontAsset>,
    /// Image/SVG files to load.
    #[serde(default)]
    pub images: Vec<ImageAsset>,
}

/// A font file to load, referenced by `font_id` on text objects.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FontAsset {
    /// Unique asset id.
    pub id: String,
    /// Filesystem path, resolved against the working directory
    /// (the CLI) or `LUMINA_ASSET_ROOT` (the server).
    pub path: String,
}

/// A raster image or SVG file to load, referenced by `asset_id`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ImageAsset {
    /// Unique asset id.
    pub id: String,
    /// Filesystem path, resolved against the working directory
    /// (the CLI) or `LUMINA_ASSET_ROOT` (the server).
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", content = "properties")]
/// Every drawable object type, tagged by `"type"` in JSON with the
/// payload under `"properties"`.
pub enum Object {
    /// A circle.
    Circle(CircleProps),
    /// A rectangle (optionally rounded).
    Rectangle(RectangleProps),
    /// A closed polygon.
    Polygon(PolygonProps),
    /// An SVG-path shape.
    Path(PathProps),
    /// A straight line segment.
    Line(LineProps),
    /// An arrow with a head.
    Arrow(ArrowProps),
    /// A text run.
    Text(TextProps),
    /// Math text (Unicode substitution).
    LaTeX(LaTeXProps),
    /// A transform group of other objects.
    Group(GroupProps),
    /// A raster image asset.
    Image(ImageProps),
    /// A rasterized SVG asset.
    SVG(SVGProps),
    /// A number line with ticks.
    NumberLine(NumberLineProps),
    /// A 2-D coordinate system.
    Axes(AxesProps),
    /// A function graph over an axes object.
    Plot(PlotProps),
    /// A cubic Bézier stroke.
    BezierCurve(BezierCurveProps),
    /// `MathML` markup (Unicode substitution).
    MathML(MathMLProps),
    /// A deterministic particle emitter.
    Particles(ParticlesProps),
}

/// A paint source for a fill or stroke. Backward compatible with a plain hex
/// string (`"#RRGGBB"`); may also be a linear or radial gradient.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Paint {
    /// A solid hex color (`#rgb`, `#rrggbb`, or `#rrggbbaa`).
    Solid(String),
    /// A linear or radial gradient.
    Gradient(GradientSpec),
}

impl Default for Paint {
    fn default() -> Self {
        Paint::Solid("#FFFFFF".to_string())
    }
}

impl From<&str> for Paint {
    fn from(s: &str) -> Self {
        Paint::Solid(s.to_string())
    }
}

impl From<String> for Paint {
    fn from(s: String) -> Self {
        Paint::Solid(s)
    }
}

/// A linear (`"type": "linear"`, with `angle` in degrees) or radial
/// (`"type": "radial"`, with fractional `radius`) gradient. `stops` is a list
/// of `[position (0..1), "#RRGGBB"]` pairs.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GradientSpec {
    /// Gradient type: `"linear"` (default) or `"radial"`.
    #[serde(rename = "type")]
    pub kind: String,
    /// Gradient stops as `[position, "#hex"]` pairs (positions in `[0, 1]`).
    pub stops: Vec<(f32, String)>,
    /// Linear gradient direction in degrees (0 = left→right).
    #[serde(default)]
    pub angle: f32,
    /// Radius in pixels.
    #[serde(default)]
    pub radius: Option<f32>,
}

/// An optional drop shadow / glow for a shape. Opt-in: rendering cost is only
/// paid when present.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Shadow {
    /// Stroke/text color as a hex string.
    #[serde(default = "default_stroke")]
    pub color: String,
    /// Blur radius in pixels (0 = hard shadow).
    #[serde(default)]
    pub blur: f32,
    /// Horizontal shadow offset in pixels.
    #[serde(default)]
    pub dx: f32,
    /// Vertical shadow offset in pixels.
    #[serde(default)]
    pub dy: f32,
    /// Opacity in `[0, 1]`; 1 is fully opaque.
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

/// A filled/stroked circle.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CircleProps {
    /// Center x, in canvas pixels.
    pub cx: f32,
    /// Center y, in canvas pixels.
    pub cy: f32,
    /// Radius in pixels.
    pub radius: f32,
    /// Draw order; higher values draw on top.
    #[serde(default)]
    pub z_index: i32,
    /// Fill paint (solid hex or gradient). Defaults to white.
    #[serde(default)]
    pub fill: Paint,
    /// Optional stroke paint (solid hex or gradient).
    #[serde(default)]
    pub stroke: Option<Paint>,
    /// Stroke width in pixels.
    #[serde(default)]
    pub stroke_width: f32,
    /// Optional drop shadow.
    #[serde(default)]
    pub shadow: Option<Shadow>,
    /// Opacity in `[0, 1]`; 1 is fully opaque.
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

/// A filled/stroked rectangle, optionally with rounded corners.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RectangleProps {
    /// Left edge, in canvas pixels.
    pub x: f32,
    /// Top edge, in canvas pixels.
    pub y: f32,
    /// Width in pixels.
    pub width: f32,
    /// Height in pixels.
    pub height: f32,
    /// Draw order; higher values draw on top.
    #[serde(default)]
    pub z_index: i32,
    /// Fill paint (solid hex or gradient). Defaults to white.
    #[serde(default)]
    pub fill: Paint,
    /// Optional stroke paint (solid hex or gradient).
    #[serde(default)]
    pub stroke: Option<Paint>,
    /// Stroke width in pixels.
    #[serde(default)]
    pub stroke_width: f32,
    /// Horizontal corner radius (rounded rectangle). 0 = sharp corners.
    #[serde(default)]
    pub rx: f32,
    /// Vertical corner radius. Falls back to `rx` when 0.
    #[serde(default)]
    pub ry: f32,
    /// Optional drop shadow.
    #[serde(default)]
    pub shadow: Option<Shadow>,
    /// Opacity in `[0, 1]`; 1 is fully opaque.
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

/// A closed polygon through explicit points.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PolygonProps {
    /// Vertices in order; the polygon closes automatically.
    pub points: Vec<[f32; 2]>,
    /// Draw order; higher values draw on top.
    #[serde(default)]
    pub z_index: i32,
    /// Fill paint (solid hex or gradient). Defaults to white.
    #[serde(default)]
    pub fill: Paint,
    /// Optional stroke paint (solid hex or gradient).
    #[serde(default)]
    pub stroke: Option<Paint>,
    /// Stroke width in pixels.
    #[serde(default)]
    pub stroke_width: f32,
    /// Optional drop shadow.
    #[serde(default)]
    pub shadow: Option<Shadow>,
    /// Opacity in `[0, 1]`; 1 is fully opaque.
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

/// An SVG-path shape (`M/L/H/V/C/Z` commands).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PathProps {
    /// SVG path data (`M/L/H/V/C/Z` commands).
    pub d: String,
    /// Draw order; higher values draw on top.
    #[serde(default)]
    pub z_index: i32,
    /// Fill paint (solid hex or gradient). Defaults to white.
    #[serde(default)]
    pub fill: Paint,
    /// Optional stroke paint (solid hex or gradient).
    #[serde(default)]
    pub stroke: Option<Paint>,
    /// Stroke width in pixels.
    #[serde(default)]
    pub stroke_width: f32,
    /// Progressive reveal: 0 = nothing drawn, 1 = complete.
    #[serde(default)]
    pub draw_fraction: Option<f32>,
    /// Optional drop shadow.
    #[serde(default)]
    pub shadow: Option<Shadow>,
    /// Opacity in `[0, 1]`; 1 is fully opaque.
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

/// A straight stroked line segment.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LineProps {
    /// Start point x.
    pub x1: f32,
    /// Start point y.
    pub y1: f32,
    /// End point x.
    pub x2: f32,
    /// End point y.
    pub y2: f32,
    /// Draw order; higher values draw on top.
    #[serde(default)]
    pub z_index: i32,
    /// Stroke color as a hex string.
    /// Stroke color as a hex string.
    #[serde(default = "default_stroke")]
    pub stroke: String,
    /// Stroke width in pixels.
    #[serde(default = "default_stroke_width")]
    pub stroke_width: f32,
    /// Dash pattern `[on, off, …]` in pixels (not yet implemented, TD-19).
    #[serde(default)]
    pub dash: Option<Vec<f32>>,
    /// Progressive reveal: 0 = nothing drawn, 1 = complete.
    #[serde(default)]
    pub draw_fraction: Option<f32>,
    /// Opacity in `[0, 1]`; 1 is fully opaque.
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

/// A stroked arrow with an arrowhead at `to`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ArrowProps {
    /// Tail position `[x, y]`.
    pub from: [f32; 2],
    /// Head (arrowhead) position `[x, y]`.
    pub to: [f32; 2],
    /// Draw order; higher values draw on top.
    #[serde(default)]
    pub z_index: i32,
    /// Stroke/text color as a hex string.
    #[serde(default = "default_stroke")]
    pub color: String,
    /// Stroke width in pixels.
    #[serde(default = "default_stroke_width")]
    pub stroke_width: f32,
    /// Optional text label near the arrow.
    #[serde(default)]
    pub label: Option<String>,
    /// Opacity in `[0, 1]`; 1 is fully opaque.
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

/// A text run drawn with a loaded font.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TextProps {
    /// The text to draw.
    pub content: String,
    /// Anchor x (interpreted per `align`).
    pub x: f32,
    /// Baseline y.
    pub y: f32,
    /// Id of a loaded font asset; falls back to the first loaded font.
    pub font_id: Option<String>,
    /// Font size in pixels.
    pub font_size: f32,
    /// Draw order; higher values draw on top.
    #[serde(default)]
    pub z_index: i32,
    /// Stroke/text color as a hex string.
    #[serde(default = "default_fill")]
    pub color: String,
    /// Horizontal anchoring at (x, y): "left" (default), "center", or "right".
    #[serde(default = "default_align")]
    pub align: String,
    /// Extra spacing added after each glyph, in pixels.
    #[serde(default)]
    pub letter_spacing: f32,
    /// Opacity in `[0, 1]`; 1 is fully opaque.
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

/// Math text rendered via Unicode substitution (see TD-06).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LaTeXProps {
    /// The math expression source text.
    pub expression: String,
    /// Anchor x (interpreted per `align`).
    pub x: f32,
    /// Baseline y.
    pub y: f32,
    /// Font size in pixels.
    pub font_size: f32,
    /// Draw order; higher values draw on top.
    #[serde(default)]
    pub z_index: i32,
    /// Stroke/text color as a hex string.
    #[serde(default = "default_fill")]
    pub color: String,
    /// Animate write-on: 0.0 = empty, 1.0 = full expression rendered.
    #[serde(default)]
    pub draw_fraction: Option<f32>,
    /// Horizontal anchoring at (x, y): "left" (default), "center", or "right".
    #[serde(default = "default_align")]
    pub align: String,
    /// Extra spacing added after each glyph, in pixels.
    #[serde(default)]
    pub letter_spacing: f32,
    /// Opacity in `[0, 1]`; 1 is fully opaque.
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

/// A transform group: children draw relative to the group's
/// translate/scale/rotation.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GroupProps {
    /// Ids of the objects belonging to this group.
    pub children: Vec<String>,
    /// Group translation x.
    pub x: f32,
    /// Group translation y.
    pub y: f32,
    /// Draw order; higher values draw on top.
    #[serde(default)]
    pub z_index: i32,
    /// Uniform scale factor (1 = unscaled).
    #[serde(default = "default_scale")]
    pub scale: f32,
    /// Rotation in degrees.
    #[serde(default)]
    pub rotation: f32,
    /// Opacity in `[0, 1]`; 1 is fully opaque.
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

/// One timeline keyframe: property values for an object at a time.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TimelineEntry {
    /// Time in seconds.
    pub time: f32,
    /// Target object id.
    pub object: String,
    /// Property values to key at this time.
    pub state: serde_json::Value,
    /// Easing name (see `lumina_core::easing::EASING_NAMES`).
    #[serde(default = "default_easing")]
    pub easing: String,
    /// Parameters for `cubic_bezier`/`spline` easings.
    #[serde(default)]
    pub easing_params: Option<serde_json::Value>,
}

/// Binds a host interaction on an object to an action.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EventEntry {
    /// Target object id.
    pub object: String,
    /// Trigger name delivered by the host (e.g. `"click"`).
    pub trigger: String,
    /// Action to perform when triggered.
    pub action: Action,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type")]
/// What an event does when triggered, tagged by `"type"` in JSON.
pub enum Action {
    /// Seek the playhead to an absolute time.
    #[serde(rename = "jump_to_time")]
    JumpToTime {
        /// Target time in seconds.
        value: f32,
    },
    /// Set an object property immediately.
    #[serde(rename = "set_property")]
    SetProperty {
        /// Id of the object to modify.
        target: String,
        /// Property name to set.
        property: String,
        /// New value.
        value: serde_json::Value,
    },
    /// Start playback from a specific time.
    #[serde(rename = "play_from")]
    PlayFrom {
        /// Time in seconds to start playback from.
        value: f32,
    },
    /// Pause playback.
    #[serde(rename = "pause")]
    Pause,
    /// Animate a target property to a new value over `duration` seconds,
    /// starting from the current playhead.
    #[serde(rename = "tween_to")]
    TweenTo {
        /// Id of the object to animate.
        target: String,
        /// Property name to animate.
        property: String,
        /// Target value.
        value: serde_json::Value,
        /// Tween length in seconds.
        #[serde(default = "default_tween_duration")]
        duration: f32,
        /// Easing name for the tween.
        #[serde(default = "default_easing")]
        easing: String,
    },
    /// Display a transient text overlay (handled by the host).
    #[serde(rename = "show_tooltip")]
    ShowTooltip {
        /// Tooltip text to display.
        text: String,
    },
    /// Emit a named custom event to the host application, with an optional
    /// payload (drag placeholders like `$drag.from` are substituted at runtime).
    #[serde(rename = "emit_custom")]
    EmitCustom {
        /// Name of the custom event to emit.
        event_name: String,
        /// Payload forwarded to the host (`$drag.*` placeholders are
        /// substituted at runtime).
        #[serde(default)]
        payload: serde_json::Value,
    },
}

fn default_tween_duration() -> f32 {
    0.5
}

/// Scene camera: a timeline of pan/zoom states.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Camera {
    /// Camera keyframes, in any order.
    pub timeline: Vec<CameraTimelineEntry>,
}

/// One camera keyframe.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CameraTimelineEntry {
    /// Time in seconds.
    pub time: f32,
    /// Camera state at this keyframe.
    pub state: CameraState,
    /// Easing name (see `lumina_core::easing::EASING_NAMES`).
    #[serde(default = "default_easing")]
    pub easing: String,
}

/// Camera pan/zoom at one instant.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CameraState {
    /// Horizontal pan in pixels.
    pub x: f32,
    /// Vertical pan in pixels.
    pub y: f32,
    /// Zoom factor about the canvas center (1 = unzoomed).
    pub zoom: f32,
}

/// A raster image (PNG/JPEG/WebP/animated GIF) placed on the canvas.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ImageProps {
    /// Id of the asset (from the scene `assets` block) to draw.
    pub asset_id: String,
    /// Left edge, in canvas pixels.
    pub x: f32,
    /// Top edge, in canvas pixels.
    pub y: f32,
    /// Target width in pixels (natural size when omitted).
    #[serde(default)]
    pub width: Option<f32>,
    /// Target height in pixels (natural size when omitted).
    #[serde(default)]
    pub height: Option<f32>,
    /// Rotation in degrees, applied about the image's center.
    #[serde(default)]
    pub rotation: f32,
    /// Draw order; higher values draw on top.
    #[serde(default)]
    pub z_index: i32,
    /// Opacity in `[0, 1]`; 1 is fully opaque.
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

/// An SVG asset rasterized and placed on the canvas.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SVGProps {
    /// Id of the asset (from the scene `assets` block) to draw.
    pub asset_id: String,
    /// Left edge, in canvas pixels.
    pub x: f32,
    /// Top edge, in canvas pixels.
    pub y: f32,
    /// Rasterization width in pixels (natural size when omitted).
    #[serde(default)]
    pub width: Option<f32>,
    /// Rasterization height in pixels (natural size when omitted).
    #[serde(default)]
    pub height: Option<f32>,
    /// Rotation in degrees, applied about the rasterized image's center.
    #[serde(default)]
    pub rotation: f32,
    /// Draw order; higher values draw on top.
    #[serde(default)]
    pub z_index: i32,
    /// Opacity in `[0, 1]`; 1 is fully opaque.
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

/// A horizontal number line with evenly spaced ticks.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NumberLineProps {
    /// First value on the line.
    pub start: f32,
    /// Last value on the line.
    pub end: f32,
    /// Distance between ticks, in value units.
    pub step: f32,
    /// Screen x of the line start.
    pub x: f32,
    /// Screen y of the line.
    pub y: f32,
    /// Total length in pixels (defaults to a standard length).
    #[serde(default)]
    pub length: Option<f32>,
    /// Draw order; higher values draw on top.
    #[serde(default)]
    pub z_index: i32,
    /// Stroke/text color as a hex string.
    #[serde(default = "default_stroke")]
    pub color: String,
    /// Opacity in `[0, 1]`; 1 is fully opaque.
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

/// A 2-D coordinate system with ticks and optional grid.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AxesProps {
    /// Mathematical x interval `[min, max]`.
    pub x_range: [f32; 2],
    /// Mathematical y interval `[min, max]`.
    pub y_range: [f32; 2],
    /// Screen x of the x-range minimum.
    pub x: f32,
    /// Screen y of the y-range minimum.
    pub y: f32,
    /// Pixels per mathematical unit.
    #[serde(default = "default_axes_scale")]
    pub scale: f32,
    /// Tick spacing along x, in value units.
    #[serde(default = "default_axis_step")]
    pub x_step: f32,
    /// Tick spacing along y, in value units.
    #[serde(default = "default_axis_step")]
    pub y_step: f32,
    /// Optional label for the x axis.
    #[serde(default)]
    pub x_label: Option<String>,
    /// Optional label for the y axis.
    #[serde(default)]
    pub y_label: Option<String>,
    /// Draw faint grid lines at every tick.
    #[serde(default)]
    pub grid: bool,
    /// Draw order; higher values draw on top.
    #[serde(default)]
    pub z_index: i32,
    /// Stroke/text color as a hex string.
    #[serde(default = "default_stroke")]
    pub color: String,
    /// Opacity in `[0, 1]`; 1 is fully opaque.
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

/// A function graph `y = f(x)` sampled over its axes' x-range.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PlotProps {
    /// Expression in `x` evaluated per sample (evalexpr syntax).
    pub function_str: String,
    /// Id of the `Axes` object that provides the coordinate system.
    pub axes_id: String,
    /// Stroke/text color as a hex string.
    #[serde(default = "default_stroke")]
    pub color: String,
    /// Stroke width in pixels.
    #[serde(default = "default_stroke_width")]
    pub stroke_width: f32,
    /// Number of samples across the x-range.
    #[serde(default = "default_sample_count")]
    pub sample_count: u32,
    /// Draw order; higher values draw on top.
    #[serde(default)]
    pub z_index: i32,
    /// Progressive reveal: 0 = nothing drawn, 1 = complete.
    #[serde(default)]
    pub draw_fraction: Option<f32>,
    /// Opacity in `[0, 1]`; 1 is fully opaque.
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

/// A single cubic Bézier stroke through four control points.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BezierCurveProps {
    /// First control point `[x, y]`.
    pub p0: [f32; 2],
    /// Second control point `[x, y]`.
    pub p1: [f32; 2],
    /// Third control point `[x, y]`.
    pub p2: [f32; 2],
    /// Fourth control point `[x, y]`.
    pub p3: [f32; 2],
    /// Stroke color as a hex string.
    #[serde(default = "default_stroke")]
    pub stroke: String,
    /// Stroke width in pixels.
    #[serde(default = "default_stroke_width")]
    pub stroke_width: f32,
    /// Draw order; higher values draw on top.
    #[serde(default)]
    pub z_index: i32,
    /// Progressive reveal: 0 = nothing drawn, 1 = complete.
    #[serde(default)]
    pub draw_fraction: Option<f32>,
    /// Opacity in `[0, 1]`; 1 is fully opaque.
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

/// Presentation `MathML` rendered through the same Unicode text fallback as
/// LaTeX. `markup` is a `MathML` string; tags are stripped and content shown.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MathMLProps {
    /// `MathML` markup source.
    pub markup: String,
    /// Anchor x (interpreted per `align`).
    pub x: f32,
    /// Baseline y.
    pub y: f32,
    /// Font size in pixels.
    pub font_size: f32,
    /// Draw order; higher values draw on top.
    #[serde(default)]
    pub z_index: i32,
    /// Stroke/text color as a hex string.
    #[serde(default = "default_fill")]
    pub color: String,
    /// Horizontal anchoring at (x, y): `"left"`, `"center"`, or `"right"`.
    #[serde(default = "default_align")]
    pub align: String,
    /// Extra spacing added after each glyph, in pixels.
    #[serde(default)]
    pub letter_spacing: f32,
    /// Opacity in `[0, 1]`; 1 is fully opaque.
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

/// A deterministic particle emitter. Particles are simulated analytically from
/// the current time + a per-particle seed, so rendering stays reproducible.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ParticlesProps {
    /// Number of particles.
    pub count: u32,
    /// Emitter x position.
    pub emitter_x: f32,
    /// Emitter y position.
    pub emitter_y: f32,
    /// Per-particle lifetime in seconds.
    #[serde(default = "default_particle_lifetime")]
    pub lifetime: f32,
    /// Initial particle speed, pixels/second.
    #[serde(default = "default_particle_speed")]
    pub speed: f32,
    /// Emission cone half-angle in degrees (360 = omnidirectional).
    #[serde(default = "default_particle_spread")]
    pub spread: f32,
    /// Particle radius in pixels.
    #[serde(default = "default_particle_size")]
    pub size: f32,
    /// Stroke/text color as a hex string.
    #[serde(default = "default_stroke")]
    pub color: String,
    /// Draw order; higher values draw on top.
    #[serde(default)]
    pub z_index: i32,
    /// Opacity in `[0, 1]`; 1 is fully opaque.
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

fn default_fill() -> String {
    "#FFFFFF".to_string()
}
fn default_stroke() -> String {
    "#FFFFFF".to_string()
}
fn default_stroke_width() -> f32 {
    1.0
}
fn default_opacity() -> f32 {
    1.0
}
fn default_scale() -> f32 {
    1.0
}
fn default_axes_scale() -> f32 {
    40.0
}
fn default_axis_step() -> f32 {
    1.0
}
fn default_easing() -> String {
    "linear".to_string()
}
fn default_align() -> String {
    "left".to_string()
}
fn default_particle_lifetime() -> f32 {
    2.0
}
fn default_particle_speed() -> f32 {
    120.0
}
fn default_particle_spread() -> f32 {
    360.0
}
fn default_particle_size() -> f32 {
    3.0
}
fn default_sample_count() -> u32 {
    200
}
