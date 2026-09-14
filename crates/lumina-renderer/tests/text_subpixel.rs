//! Text moves continuously, not a pixel at a time (`AAA-OUT-09`).
//!
//! `tiny_skia::Pixmap::draw_pixmap` snaps a translation to whole pixels
//! regardless of filter quality, so text composited that way had **one
//! position per pixel**. A caption drifting across the screen jumped a whole
//! pixel at a time; worse, each glyph crossed its own pixel boundary at a
//! different moment, so the spacing between letters visibly wobbled while the
//! word moved. Sweeping a two-glyph run across one pixel produced four
//! distinct frames instead of a continuum.
//!
//! The sub-pixel remainder is now baked into the glyph coverage and only the
//! whole-pixel part reaches the transform, which is all `draw_pixmap` would
//! have honoured anyway.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};

use luminafx_core::{SceneGraph, Timeline};
use luminafx_renderer::{skia_backend::SkiaRenderer, Renderer};
use luminafx_schema::Scene;

const W: u32 = 128;
const H: u32 = 40;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crate lives two levels below the workspace root")
        .to_path_buf()
}

/// A short word at a given x. Two glyphs, because one glyph cannot show the
/// spacing wobble that made the old behaviour worse than a simple snap.
fn frame_at(x: f64) -> Vec<u8> {
    let scene: Scene = serde_json::from_value(serde_json::json!({
        "version": "1.0",
        "meta": { "title": "subpixel", "author": "t",
                  "created_at": "2026-09-03T00:00:00Z" },
        "canvas": { "width": W, "height": H, "fps": 30, "duration": 1.0,
                    "background": "#000000" },
        "assets": { "fonts": [
            { "id": "f", "path": "examples/assets/fonts/LiberationSans-Regular.ttf" }
        ] },
        "objects": { "t": { "type": "Text", "properties": {
            "content": "Hi", "x": x, "y": 30.0, "font_size": 22.0,
            "color": "#FFFFFF", "font_id": "f", "z_index": 1 } } },
        "timeline": []
    }))
    .expect("scene");

    let graph = SceneGraph::from_scene(&scene);
    let timeline = Timeline::from_scene(&scene);
    let mut renderer = SkiaRenderer::new();
    for font in &scene.assets.fonts {
        let data = std::fs::read(workspace_root().join(&font.path)).expect("font");
        renderer.load_font(&font.id, &data).expect("load font");
    }
    renderer
        .render_frame(
            &graph.objects,
            &timeline.get_state_at(0.0),
            W,
            H,
            &scene.canvas.background,
            None,
        )
        .expect("render")
}

/// Horizontal centroid of the ink, weighted by brightness.
///
/// Brightness, not alpha: the background is opaque, so every pixel in the
/// frame has alpha 255 and an alpha-weighted centroid is just the centre of
/// the canvas — a measurement that reports "nothing moved" no matter what the
/// renderer does. White text on black makes the red channel the coverage.
fn centroid_x(frame: &[u8]) -> f64 {
    let (mut sum, mut weight) = (0.0f64, 0.0f64);
    for y in 0..H {
        for x in 0..W {
            let v = f64::from(frame[((y * W + x) * 4) as usize]);
            sum += (f64::from(x) + 0.5) * v;
            weight += v;
        }
    }
    assert!(weight > 0.0, "nothing was drawn");
    sum / weight
}

#[test]
fn every_sub_pixel_step_changes_the_image() {
    // The direct statement of the defect. Ten positions inside one pixel must
    // give ten different frames; snapping gave four.
    let steps: Vec<f64> = (0..10).map(|i| 20.0 + f64::from(i) / 10.0).collect();
    let frames: Vec<Vec<u8>> = steps.iter().map(|x| frame_at(*x)).collect();

    for (i, a) in frames.iter().enumerate() {
        for (j, b) in frames.iter().enumerate().skip(i + 1) {
            assert_ne!(
                a, b,
                "x={} and x={} render identically — text is snapping to whole pixels",
                steps[i], steps[j]
            );
        }
    }
}

#[test]
fn the_text_moves_monotonically_with_its_x() {
    // Distinctness alone would be satisfied by frames that differ arbitrarily.
    // What makes it *motion* is that the ink moves right when x does, by about
    // the amount asked for.
    let steps: Vec<f64> = (0..10).map(|i| 20.0 + f64::from(i) / 10.0).collect();
    let centroids: Vec<f64> = steps.iter().map(|x| centroid_x(&frame_at(*x))).collect();

    for w in centroids.windows(2) {
        assert!(
            w[1] > w[0],
            "the text did not move right when x increased: {centroids:?}"
        );
    }

    // Over nine tenths of a pixel the centroid should travel about that far.
    let travelled = centroids[9] - centroids[0];
    assert!(
        (travelled - 0.9).abs() < 0.25,
        "the text travelled {travelled:.3}px while x moved 0.9px"
    );
}

#[test]
fn a_whole_pixel_step_moves_by_exactly_one_pixel() {
    // The sanity check on the other side: sub-pixel handling must not have
    // disturbed the integer case, where the answer is exact.
    let a = centroid_x(&frame_at(20.0));
    let b = centroid_x(&frame_at(21.0));
    assert!(
        (b - a - 1.0).abs() < 0.02,
        "a one-pixel move shifted the ink by {:.4}px",
        b - a
    );
}
