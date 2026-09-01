//! Shared scene-walk helpers: z-ordering and group transform composition.

use lumina_schema::Object;
use serde_json::Value;
use std::collections::HashMap;

/// The z-index of any object variant.
pub(crate) fn z_index(obj: &Object) -> i32 {
    match obj {
        Object::Circle(p) => p.z_index,
        Object::Rectangle(p) => p.z_index,
        Object::Polygon(p) => p.z_index,
        Object::Path(p) => p.z_index,
        Object::Line(p) => p.z_index,
        Object::Arrow(p) => p.z_index,
        Object::Text(p) => p.z_index,
        Object::LaTeX(p) => p.z_index,
        Object::Group(p) => p.z_index,
        Object::Image(p) => p.z_index,
        Object::SVG(p) => p.z_index,
        Object::NumberLine(p) => p.z_index,
        Object::Axes(p) => p.z_index,
        Object::Plot(p) => p.z_index,
        Object::BezierCurve(p) => p.z_index,
        Object::MathML(p) => p.z_index,
        Object::Particles(p) => p.z_index,
    }
}

/// Ids of root objects (those not claimed as a child by any group), sorted
/// by z-index ascending. The sort is stable, so ties keep the map's
/// iteration order — identical for both backends within a frame because
/// they receive the same map.
pub(crate) fn sorted_root_ids(objects: &HashMap<String, Object>) -> Vec<String> {
    let mut child_ids = std::collections::HashSet::new();
    for obj in objects.values() {
        if let Object::Group(group) = obj {
            for child_id in &group.children {
                child_ids.insert(child_id.clone());
            }
        }
    }

    let mut roots: Vec<(String, i32)> = objects
        .iter()
        .filter(|(id, _)| !child_ids.contains(*id))
        .map(|(id, obj)| (id.clone(), z_index(obj)))
        .collect();

    roots.sort_by_key(|(_, z)| *z);
    roots.into_iter().map(|(id, _)| id).collect()
}

/// A group's children sorted by z-index ascending (missing children get 0).
pub(crate) fn sorted_children<'a>(
    children: &'a [String],
    objects: &HashMap<String, Object>,
) -> Vec<&'a str> {
    let mut out: Vec<(&str, i32)> = children
        .iter()
        .map(|cid| (cid.as_str(), objects.get(cid).map(z_index).unwrap_or(0)))
        .collect();
    out.sort_by_key(|(_, z)| *z);
    out.into_iter().map(|(id, _)| id).collect()
}

/// A row-major 2×3 affine matrix `[sx, kx, tx, ky, sy, ty]` — the single
/// source of truth for transform math, converted losslessly to each
/// backend's type. Layout matches tiny-skia's `Transform` fields.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Mat2x3 {
    pub sx: f32,
    pub kx: f32,
    pub ky: f32,
    pub sy: f32,
    pub tx: f32,
    pub ty: f32,
}

impl Mat2x3 {
    pub(crate) const IDENTITY: Self = Self {
        sx: 1.0,
        kx: 0.0,
        ky: 0.0,
        sy: 1.0,
        tx: 0.0,
        ty: 0.0,
    };

    pub(crate) fn from_tiny(t: tiny_skia::Transform) -> Self {
        Self {
            sx: t.sx,
            kx: t.kx,
            ky: t.ky,
            sy: t.sy,
            tx: t.tx,
            ty: t.ty,
        }
    }

    pub(crate) fn to_tiny(self) -> tiny_skia::Transform {
        tiny_skia::Transform::from_row(self.sx, self.ky, self.kx, self.sy, self.tx, self.ty)
    }

    pub(crate) fn to_kurbo(self) -> vello::kurbo::Affine {
        // kurbo coefficient order: [sx, ky, kx, sy, tx, ty]
        vello::kurbo::Affine::new([
            self.sx as f64,
            self.ky as f64,
            self.kx as f64,
            self.sy as f64,
            self.tx as f64,
            self.ty as f64,
        ])
    }
}

/// The root camera transform: zoom about the canvas center, then pan.
/// Computed in f32 via tiny-skia so the matrix is bit-identical on both
/// backends.
pub(crate) fn camera_transform(
    camera: Option<&lumina_schema::CameraState>,
    width: u32,
    height: u32,
) -> Mat2x3 {
    match camera {
        Some(cam) => {
            let cx = width as f32 / 2.0;
            let cy = height as f32 / 2.0;
            let t = tiny_skia::Transform::from_translate(cx + cam.x, cy + cam.y)
                .pre_concat(tiny_skia::Transform::from_scale(cam.zoom, cam.zoom))
                .pre_concat(tiny_skia::Transform::from_translate(-cx, -cy));
            Mat2x3::from_tiny(t)
        }
        None => Mat2x3::IDENTITY,
    }
}

/// Compose a group's local transform onto `parent`: translate(x, y), then
/// scale, then rotation (degrees), exactly as both backends have always
/// done — computed in f32 via tiny-skia so the matrix is bit-identical on
/// both backends.
pub(crate) fn group_transform(parent: Mat2x3, state: &Value) -> Mat2x3 {
    let x = state["x"].as_f64().unwrap_or(0.0) as f32;
    let y = state["y"].as_f64().unwrap_or(0.0) as f32;
    let scale = state["scale"].as_f64().unwrap_or(1.0) as f32;
    let rotation_deg = state["rotation"].as_f64().unwrap_or(0.0) as f32;

    let mut t = parent.to_tiny();
    t = t.pre_translate(x, y);
    if scale != 1.0 {
        t = t.pre_scale(scale, scale);
    }
    if rotation_deg != 0.0 {
        t = t.pre_rotate(rotation_deg);
    }
    Mat2x3::from_tiny(t)
}
