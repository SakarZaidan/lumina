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

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Scene {
    pub version: String,
    pub meta: Meta,
    pub canvas: Canvas,
    #[serde(default)]
    pub assets: Assets,
    pub objects: HashMap<String, Object>,
    pub timeline: Vec<TimelineEntry>,
    #[serde(default)]
    pub events: Vec<EventEntry>,
    #[serde(default)]
    pub camera: Option<Camera>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Meta {
    pub title: String,
    pub author: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Canvas {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub duration: f32,
    pub background: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Assets {
    #[serde(default)]
    pub fonts: Vec<FontAsset>,
    #[serde(default)]
    pub images: Vec<ImageAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FontAsset {
    pub id: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ImageAsset {
    pub id: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", content = "properties")]
pub enum Object {
    Circle(CircleProps),
    Rectangle(RectangleProps),
    Polygon(PolygonProps),
    Path(PathProps),
    Line(LineProps),
    Arrow(ArrowProps),
    Text(TextProps),
    LaTeX(LaTeXProps),
    Group(GroupProps),
    Image(ImageProps),
    SVG(SVGProps),
    NumberLine(NumberLineProps),
    Axes(AxesProps),
    Plot(PlotProps),
    BezierCurve(BezierCurveProps),
    MathML(MathMLProps),
    Particles(ParticlesProps),
}

/// A paint source for a fill or stroke. Backward compatible with a plain hex
/// string (`"#RRGGBB"`); may also be a linear or radial gradient.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Paint {
    Solid(String),
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
    #[serde(rename = "type")]
    pub kind: String,
    pub stops: Vec<(f32, String)>,
    #[serde(default)]
    pub angle: f32,
    #[serde(default)]
    pub radius: Option<f32>,
}

/// An optional drop shadow / glow for a shape. Opt-in: rendering cost is only
/// paid when present.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Shadow {
    #[serde(default = "default_stroke")]
    pub color: String,
    #[serde(default)]
    pub blur: f32,
    #[serde(default)]
    pub dx: f32,
    #[serde(default)]
    pub dy: f32,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CircleProps {
    pub cx: f32,
    pub cy: f32,
    pub radius: f32,
    #[serde(default)]
    pub z_index: i32,
    #[serde(default)]
    pub fill: Paint,
    #[serde(default)]
    pub stroke: Option<Paint>,
    #[serde(default)]
    pub stroke_width: f32,
    #[serde(default)]
    pub shadow: Option<Shadow>,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RectangleProps {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    #[serde(default)]
    pub z_index: i32,
    #[serde(default)]
    pub fill: Paint,
    #[serde(default)]
    pub stroke: Option<Paint>,
    #[serde(default)]
    pub stroke_width: f32,
    /// Horizontal corner radius (rounded rectangle). 0 = sharp corners.
    #[serde(default)]
    pub rx: f32,
    /// Vertical corner radius. Falls back to `rx` when 0.
    #[serde(default)]
    pub ry: f32,
    #[serde(default)]
    pub shadow: Option<Shadow>,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PolygonProps {
    pub points: Vec<[f32; 2]>,
    #[serde(default)]
    pub z_index: i32,
    #[serde(default)]
    pub fill: Paint,
    #[serde(default)]
    pub stroke: Option<Paint>,
    #[serde(default)]
    pub stroke_width: f32,
    #[serde(default)]
    pub shadow: Option<Shadow>,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PathProps {
    pub d: String,
    #[serde(default)]
    pub z_index: i32,
    #[serde(default)]
    pub fill: Paint,
    #[serde(default)]
    pub stroke: Option<Paint>,
    #[serde(default)]
    pub stroke_width: f32,
    #[serde(default)]
    pub draw_fraction: Option<f32>,
    #[serde(default)]
    pub shadow: Option<Shadow>,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LineProps {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    #[serde(default)]
    pub z_index: i32,
    #[serde(default = "default_stroke")]
    pub stroke: String,
    #[serde(default = "default_stroke_width")]
    pub stroke_width: f32,
    #[serde(default)]
    pub dash: Option<Vec<f32>>,
    #[serde(default)]
    pub draw_fraction: Option<f32>,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ArrowProps {
    pub from: [f32; 2],
    pub to: [f32; 2],
    #[serde(default)]
    pub z_index: i32,
    #[serde(default = "default_stroke")]
    pub color: String,
    #[serde(default = "default_stroke_width")]
    pub stroke_width: f32,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TextProps {
    pub content: String,
    pub x: f32,
    pub y: f32,
    pub font_id: Option<String>,
    pub font_size: f32,
    #[serde(default)]
    pub z_index: i32,
    #[serde(default = "default_fill")]
    pub color: String,
    /// Horizontal anchoring at (x, y): "left" (default), "center", or "right".
    #[serde(default = "default_align")]
    pub align: String,
    /// Extra spacing added after each glyph, in pixels.
    #[serde(default)]
    pub letter_spacing: f32,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LaTeXProps {
    pub expression: String,
    pub x: f32,
    pub y: f32,
    pub font_size: f32,
    #[serde(default)]
    pub z_index: i32,
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
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GroupProps {
    pub children: Vec<String>,
    pub x: f32,
    pub y: f32,
    #[serde(default)]
    pub z_index: i32,
    #[serde(default = "default_scale")]
    pub scale: f32,
    #[serde(default)]
    pub rotation: f32,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TimelineEntry {
    pub time: f32,
    pub object: String,
    pub state: serde_json::Value,
    #[serde(default = "default_easing")]
    pub easing: String,
    #[serde(default)]
    pub easing_params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EventEntry {
    pub object: String,
    pub trigger: String,
    pub action: Action,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type")]
pub enum Action {
    #[serde(rename = "jump_to_time")]
    JumpToTime { value: f32 },
    #[serde(rename = "set_property")]
    SetProperty {
        target: String,
        property: String,
        value: serde_json::Value,
    },
    /// Start playback from a specific time.
    #[serde(rename = "play_from")]
    PlayFrom { value: f32 },
    /// Pause playback.
    #[serde(rename = "pause")]
    Pause,
    /// Animate a target property to a new value over `duration` seconds,
    /// starting from the current playhead.
    #[serde(rename = "tween_to")]
    TweenTo {
        target: String,
        property: String,
        value: serde_json::Value,
        #[serde(default = "default_tween_duration")]
        duration: f32,
        #[serde(default = "default_easing")]
        easing: String,
    },
    /// Display a transient text overlay (handled by the host).
    #[serde(rename = "show_tooltip")]
    ShowTooltip { text: String },
    /// Emit a named custom event to the host application, with an optional
    /// payload (drag placeholders like `$drag.from` are substituted at runtime).
    #[serde(rename = "emit_custom")]
    EmitCustom {
        event_name: String,
        #[serde(default)]
        payload: serde_json::Value,
    },
}

fn default_tween_duration() -> f32 {
    0.5
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Camera {
    pub timeline: Vec<CameraTimelineEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CameraTimelineEntry {
    pub time: f32,
    pub state: CameraState,
    #[serde(default = "default_easing")]
    pub easing: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CameraState {
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ImageProps {
    pub asset_id: String,
    pub x: f32,
    pub y: f32,
    #[serde(default)]
    pub width: Option<f32>,
    #[serde(default)]
    pub height: Option<f32>,
    /// Rotation in degrees, applied about the image's center.
    #[serde(default)]
    pub rotation: f32,
    #[serde(default)]
    pub z_index: i32,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SVGProps {
    pub asset_id: String,
    pub x: f32,
    pub y: f32,
    #[serde(default)]
    pub width: Option<f32>,
    #[serde(default)]
    pub height: Option<f32>,
    /// Rotation in degrees, applied about the rasterized image's center.
    #[serde(default)]
    pub rotation: f32,
    #[serde(default)]
    pub z_index: i32,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NumberLineProps {
    pub start: f32,
    pub end: f32,
    pub step: f32,
    pub x: f32,
    pub y: f32,
    #[serde(default)]
    pub length: Option<f32>,
    #[serde(default)]
    pub z_index: i32,
    #[serde(default = "default_stroke")]
    pub color: String,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AxesProps {
    pub x_range: [f32; 2],
    pub y_range: [f32; 2],
    pub x: f32,
    pub y: f32,
    #[serde(default = "default_axes_scale")]
    pub scale: f32,
    #[serde(default = "default_axis_step")]
    pub x_step: f32,
    #[serde(default = "default_axis_step")]
    pub y_step: f32,
    #[serde(default)]
    pub x_label: Option<String>,
    #[serde(default)]
    pub y_label: Option<String>,
    #[serde(default)]
    pub grid: bool,
    #[serde(default)]
    pub z_index: i32,
    #[serde(default = "default_stroke")]
    pub color: String,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PlotProps {
    pub function_str: String,
    pub axes_id: String,
    #[serde(default = "default_stroke")]
    pub color: String,
    #[serde(default = "default_stroke_width")]
    pub stroke_width: f32,
    #[serde(default = "default_sample_count")]
    pub sample_count: u32,
    #[serde(default)]
    pub z_index: i32,
    #[serde(default)]
    pub draw_fraction: Option<f32>,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BezierCurveProps {
    pub p0: [f32; 2],
    pub p1: [f32; 2],
    pub p2: [f32; 2],
    pub p3: [f32; 2],
    #[serde(default = "default_stroke")]
    pub stroke: String,
    #[serde(default = "default_stroke_width")]
    pub stroke_width: f32,
    #[serde(default)]
    pub z_index: i32,
    #[serde(default)]
    pub draw_fraction: Option<f32>,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

/// Presentation MathML rendered through the same Unicode text fallback as
/// LaTeX. `markup` is a MathML string; tags are stripped and content shown.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MathMLProps {
    pub markup: String,
    pub x: f32,
    pub y: f32,
    pub font_size: f32,
    #[serde(default)]
    pub z_index: i32,
    #[serde(default = "default_fill")]
    pub color: String,
    #[serde(default = "default_align")]
    pub align: String,
    #[serde(default)]
    pub letter_spacing: f32,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

/// A deterministic particle emitter. Particles are simulated analytically from
/// the current time + a per-particle seed, so rendering stays reproducible.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ParticlesProps {
    pub count: u32,
    pub emitter_x: f32,
    pub emitter_y: f32,
    #[serde(default = "default_particle_lifetime")]
    pub lifetime: f32,
    #[serde(default = "default_particle_speed")]
    pub speed: f32,
    /// Emission cone half-angle in degrees (360 = omnidirectional).
    #[serde(default = "default_particle_spread")]
    pub spread: f32,
    #[serde(default = "default_particle_size")]
    pub size: f32,
    #[serde(default = "default_stroke")]
    pub color: String,
    #[serde(default)]
    pub z_index: i32,
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
