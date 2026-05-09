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
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CircleProps {
    pub cx: f32,
    pub cy: f32,
    pub radius: f32,
    #[serde(default)]
    pub z_index: i32,
    #[serde(default = "default_fill")]
    pub fill: String,
    #[serde(default)]
    pub stroke: Option<String>,
    #[serde(default)]
    pub stroke_width: f32,
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
    #[serde(default = "default_fill")]
    pub fill: String,
    #[serde(default)]
    pub stroke: Option<String>,
    #[serde(default)]
    pub stroke_width: f32,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PolygonProps {
    pub points: Vec<[f32; 2]>,
    #[serde(default)]
    pub z_index: i32,
    #[serde(default = "default_fill")]
    pub fill: String,
    #[serde(default)]
    pub stroke: Option<String>,
    #[serde(default)]
    pub stroke_width: f32,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PathProps {
    pub d: String,
    #[serde(default)]
    pub z_index: i32,
    #[serde(default = "default_fill")]
    pub fill: String,
    #[serde(default)]
    pub stroke: Option<String>,
    #[serde(default)]
    pub stroke_width: f32,
    #[serde(default)]
    pub draw_fraction: Option<f32>,
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

fn default_fill() -> String { "#FFFFFF".to_string() }
fn default_stroke() -> String { "#FFFFFF".to_string() }
fn default_stroke_width() -> f32 { 1.0 }
fn default_opacity() -> f32 { 1.0 }
fn default_scale() -> f32 { 1.0 }
fn default_axes_scale() -> f32 { 40.0 }
fn default_axis_step() -> f32 { 1.0 }
fn default_easing() -> String { "linear".to_string() }
fn default_sample_count() -> u32 { 200 }
