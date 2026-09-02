//! A gradient and a fade between the same two colours must agree.
//!
//! The timeline blends colours in `OKLab`; both rasterisers interpolate
//! linearly in sRGB between adjacent gradient stops. So the same two colours
//! produced one midpoint when they met in a gradient and a different one when
//! they met in an animation — two colour models in a single frame.
//!
//! Neither backend lets a caller choose the interpolation space, but both
//! accept arbitrarily many stops, so the fix is to sample the perceptual curve
//! and hand over the samples.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use luminafx_core::{SceneGraph, Timeline};
use luminafx_renderer::{skia_backend::SkiaRenderer, Renderer};
use luminafx_schema::Scene;

const W: u32 = 120;
const H: u32 = 40;

/// Render a horizontal two-stop gradient and return the centre pixel.
fn gradient_midpoint(from: &str, to: &str) -> [u8; 4] {
    let scene: Scene = serde_json::from_value(serde_json::json!({
        "version": "1.0",
        "meta": { "title": "t", "author": "a", "created_at": "2026-01-01T00:00:00Z" },
        "canvas": { "width": W, "height": H, "fps": 30, "duration": 1.0,
                    "background": "#000000" },
        "objects": {
            "r": { "type": "Rectangle", "properties": {
                "x": 0.0, "y": 0.0, "width": W as f64, "height": H as f64,
                "fill": { "type": "linear", "angle": 0,
                          "stops": [[0.0, from], [1.0, to]] },
                "z_index": 1, "opacity": 1.0 } }
        },
        "timeline": []
    }))
    .expect("fixture");

    let graph = SceneGraph::from_scene(&scene);
    let states = Timeline::from_scene(&scene).get_state_at(0.0);
    let mut r = SkiaRenderer::new();
    let px = r
        .render_frame(&graph.objects, &states, W, H, "#000000", None)
        .expect("render");
    let i = (((H / 2) * W) + (W / 2)) as usize * 4;
    [px[i], px[i + 1], px[i + 2], px[i + 3]]
}

/// The colour a keyframe fade shows halfway between the same two colours.
fn fade_midpoint(from: &str, to: &str) -> [u8; 4] {
    let mixed = luminafx_core::interpolator::interpolate_value(
        &serde_json::json!(from),
        &serde_json::json!(to),
        0.5,
        "linear",
        None,
    );
    let hex = mixed.as_str().expect("a colour").trim_start_matches('#');
    let byte = |i: usize| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).expect("hex");
    [byte(0), byte(1), byte(2), 255]
}

#[test]
fn a_gradient_midpoint_matches_a_fade_midpoint() {
    // The whole claim. A tolerance of 4 covers the piecewise-linear
    // approximation between sampled stops plus antialiasing at the sample
    // point; the difference being corrected here is far larger.
    for (from, to) in [
        ("#FF0000", "#0000FF"), // red to blue — where sRGB goes darkest
        ("#0000FF", "#FFFFFF"), // blue to white — where CIELAB drifts purple
        ("#FFFF00", "#00FFFF"),
        ("#000000", "#FFFFFF"),
    ] {
        let g = gradient_midpoint(from, to);
        let f = fade_midpoint(from, to);
        let delta = (0..3)
            .map(|i| i32::from(g[i]).abs_diff(i32::from(f[i])))
            .max();
        assert!(
            delta.unwrap_or(u32::MAX) <= 4,
            "{from} -> {to}: gradient midpoint {g:?} but fade midpoint {f:?}"
        );
    }
}

#[test]
fn a_gradient_still_starts_and_ends_on_its_own_stops() {
    // Refinement must not disturb the author's colours, only the path between.
    let scene_px = gradient_edges("#FF0000", "#0000FF");
    assert!(
        scene_px.0[0] > 200 && scene_px.0[2] < 60,
        "left edge should be red, was {:?}",
        scene_px.0
    );
    assert!(
        scene_px.1[2] > 200 && scene_px.1[0] < 60,
        "right edge should be blue, was {:?}",
        scene_px.1
    );
}

fn gradient_edges(from: &str, to: &str) -> ([u8; 4], [u8; 4]) {
    let scene: Scene = serde_json::from_value(serde_json::json!({
        "version": "1.0",
        "meta": { "title": "t", "author": "a", "created_at": "2026-01-01T00:00:00Z" },
        "canvas": { "width": W, "height": H, "fps": 30, "duration": 1.0,
                    "background": "#000000" },
        "objects": {
            "r": { "type": "Rectangle", "properties": {
                "x": 0.0, "y": 0.0, "width": W as f64, "height": H as f64,
                "fill": { "type": "linear", "angle": 0,
                          "stops": [[0.0, from], [1.0, to]] },
                "z_index": 1, "opacity": 1.0 } }
        },
        "timeline": []
    }))
    .expect("fixture");
    let graph = SceneGraph::from_scene(&scene);
    let states = Timeline::from_scene(&scene).get_state_at(0.0);
    let mut r = SkiaRenderer::new();
    let px = r
        .render_frame(&graph.objects, &states, W, H, "#000000", None)
        .expect("render");
    let row = (H / 2) * W;
    let at = |x: u32| {
        let i = (row + x) as usize * 4;
        [px[i], px[i + 1], px[i + 2], px[i + 3]]
    };
    (at(1), at(W - 2))
}

#[test]
fn the_perceptual_path_differs_from_a_naive_srgb_blend() {
    // Guards against the refinement silently doing nothing: red to blue is the
    // transition where sRGB interpolation is most obviously wrong, dipping
    // through a dark muddy purple that neither endpoint suggests.
    let g = gradient_midpoint("#FF0000", "#0000FF");
    let naive = [128u8, 0, 128, 255];
    let delta = (0..3)
        .map(|i| i32::from(g[i]).abs_diff(i32::from(naive[i])))
        .max();
    assert!(
        delta.unwrap_or(0) > 8,
        "the midpoint {g:?} is indistinguishable from a naive sRGB blend {naive:?}"
    );
}

#[test]
fn multi_stop_gradients_keep_every_author_stop() {
    // Refinement inserts stops; it must not drop or move the given ones.
    let scene: Scene = serde_json::from_value(serde_json::json!({
        "version": "1.0",
        "meta": { "title": "t", "author": "a", "created_at": "2026-01-01T00:00:00Z" },
        "canvas": { "width": W, "height": H, "fps": 30, "duration": 1.0,
                    "background": "#000000" },
        "objects": {
            "r": { "type": "Rectangle", "properties": {
                "x": 0.0, "y": 0.0, "width": W as f64, "height": H as f64,
                "fill": { "type": "linear", "angle": 0, "stops": [
                    [0.0, "#FF0000"], [0.5, "#00FF00"], [1.0, "#0000FF"]] },
                "z_index": 1, "opacity": 1.0 } }
        },
        "timeline": []
    }))
    .expect("fixture");
    let graph = SceneGraph::from_scene(&scene);
    let states = Timeline::from_scene(&scene).get_state_at(0.0);
    let mut r = SkiaRenderer::new();
    let px = r
        .render_frame(&graph.objects, &states, W, H, "#000000", None)
        .expect("render");
    let i = (((H / 2) * W) + (W / 2)) as usize * 4;
    let mid = [px[i], px[i + 1], px[i + 2]];
    assert!(
        mid[1] > 180 && mid[0] < 90 && mid[2] < 90,
        "the middle stop is green and must survive refinement, got {mid:?}"
    );
}
