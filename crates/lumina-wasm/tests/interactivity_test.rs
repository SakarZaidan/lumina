//! Browser/Node interactivity tests for the WASM engine. Run with:
//!   wasm-pack test --node crates/lumina-wasm

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use luminafx_wasm::LuminaEngine;
use serde_json::json;
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

/// Build the scene the way a real caller does.
///
/// The JS SDK passes a plain object (`new LuminaEngine(scene as object)`), which
/// is what `JSON.parse` produces. `serde_wasm_bindgen::to_value` would instead
/// emit a JS `Map`, and `Scene` does not deserialize from one — every field
/// reads as absent, so construction fails with a missing-`version` error.
fn to_js(v: &serde_json::Value) -> JsValue {
    js_sys::JSON::parse(&v.to_string()).expect("scene JSON parses")
}

#[wasm_bindgen_test]
fn test_hit_test_circle_inside_and_outside() {
    let scene = json!({
        "version": "1.0",
        "meta": { "title": "t", "author": "a", "created_at": "now" },
        "canvas": { "width": 200, "height": 200, "fps": 30, "duration": 1.0, "background": "#000000" },
        "objects": {
            "c": { "type": "Circle", "properties": { "cx": 100.0, "cy": 100.0, "radius": 40.0, "z_index": 1 } }
        },
        "timeline": []
    });
    let engine = LuminaEngine::new(to_js(&scene)).unwrap();
    assert_eq!(engine.hit_test(100.0, 100.0, 0.0).as_deref(), Some("c"));
    assert_eq!(engine.hit_test(5.0, 5.0, 0.0), None);
}

#[wasm_bindgen_test]
fn test_hit_test_polygon_ray_casting() {
    let scene = json!({
        "version": "1.0",
        "meta": { "title": "t", "author": "a", "created_at": "now" },
        "canvas": { "width": 200, "height": 200, "fps": 30, "duration": 1.0, "background": "#000000" },
        "objects": {
            "tri": { "type": "Polygon", "properties": { "points": [[10.0, 10.0], [110.0, 10.0], [10.0, 110.0]], "z_index": 1 } }
        },
        "timeline": []
    });
    let engine = LuminaEngine::new(to_js(&scene)).unwrap();
    // Inside the triangle.
    assert_eq!(engine.hit_test(30.0, 30.0, 0.0).as_deref(), Some("tri"));
    // Outside (beyond the hypotenuse).
    assert_eq!(engine.hit_test(100.0, 100.0, 0.0), None);
}

#[wasm_bindgen_test]
fn test_hit_test_group_child_through_transform() {
    let scene = json!({
        "version": "1.0",
        "meta": { "title": "t", "author": "a", "created_at": "now" },
        "canvas": { "width": 400, "height": 400, "fps": 30, "duration": 1.0, "background": "#000000" },
        "objects": {
            "g": { "type": "Group", "properties": { "x": 100.0, "y": 100.0, "z_index": 1, "children": ["dot"] } },
            "dot": { "type": "Circle", "properties": { "cx": 0.0, "cy": 0.0, "radius": 20.0, "z_index": 2 } }
        },
        "timeline": []
    });
    let engine = LuminaEngine::new(to_js(&scene)).unwrap();
    // The child sits at group-local (0,0) → world (100,100).
    assert_eq!(engine.hit_test(100.0, 100.0, 0.0).as_deref(), Some("dot"));
}
