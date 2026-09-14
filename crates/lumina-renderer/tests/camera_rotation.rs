//! The camera can rotate (`AAA-MOT-05`), and adding it changed nothing else.
//!
//! Two claims are worth a test, and neither is about matrix arithmetic:
//!
//! 1. **A zero rotation is not composed.** Every camera authored before this
//!    field existed carries `rotation: 0.0` by default, and every one of those
//!    scenes must produce the pixels it produced before — not "within a
//!    tolerance", but the identical bytes. Multiplying by a rotation matrix
//!    that is only *approximately* the identity (`cos(0)` is exact, but the
//!    concatenation still rounds) would drift the whole frame by a fraction of
//!    a pixel and silently invalidate every golden image in the repository.
//!
//! 2. **A non-zero rotation actually rotates the scene**, about the canvas
//!    centre rather than the origin. A field that parses, interpolates, and
//!    reaches the transform without moving anything would pass every other
//!    test here.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use luminafx_core::{SceneGraph, Timeline};
use luminafx_renderer::{skia_backend::SkiaRenderer, Renderer};
use luminafx_schema::Scene;

const W: u32 = 128;
const H: u32 = 128;

/// One small square directly above the canvas centre.
///
/// Off-centre and asymmetric on purpose: a shape centred on the axis of
/// rotation looks the same at every angle, which is exactly the scene that
/// cannot detect the bug.
fn scene(rotation: Option<f32>) -> Scene {
    let camera = rotation.map(|r| {
        serde_json::json!({
            "timeline": [
                { "time": 0.0, "state": { "x": 0, "y": 0, "zoom": 1.0, "rotation": r },
                  "easing": "linear" }
            ]
        })
    });
    let mut value = serde_json::json!({
        "version": "1.0",
        "meta": { "title": "camera rotation", "author": "t",
                  "created_at": "2026-09-02T00:00:00Z" },
        "canvas": { "width": W, "height": H, "fps": 30, "duration": 1.0,
                    "background": "#000000" },
        "objects": {
            "marker": {
                "type": "Rectangle",
                "properties": { "x": 54.0, "y": 14.0, "width": 20.0, "height": 20.0,
                                "fill": "#FFFFFF", "z_index": 1 }
            }
        },
        "timeline": []
    });
    if let Some(c) = camera {
        value["camera"] = c;
    }
    serde_json::from_value(value).expect("scene")
}

fn render(scene: &Scene) -> Vec<u8> {
    let graph = SceneGraph::from_scene(scene);
    let timeline = Timeline::from_scene(scene);
    let states = timeline.get_state_at(0.0);
    let camera = timeline.get_camera_at(0.0, scene);
    let mut renderer = SkiaRenderer::new();
    renderer
        .render_frame(
            &graph.objects,
            &states,
            W,
            H,
            &scene.canvas.background,
            Some(&camera),
        )
        .expect("render")
}

/// Centroid of the lit pixels, in canvas coordinates.
///
/// Measured at pixel *centres*. Indexing by the top-left corner biases the
/// answer half a pixel along both axes, which is enough to make an exact
/// distance-from-centre comparison fail by exactly the bias.
fn centroid(frame: &[u8]) -> (f32, f32) {
    let (mut sx, mut sy, mut n) = (0.0f64, 0.0f64, 0u32);
    for y in 0..H {
        for x in 0..W {
            let i = ((y * W + x) * 4) as usize;
            if frame[i] > 128 {
                sx += f64::from(x) + 0.5;
                sy += f64::from(y) + 0.5;
                n += 1;
            }
        }
    }
    assert!(n > 0, "nothing was drawn");
    ((sx / f64::from(n)) as f32, (sy / f64::from(n)) as f32)
}

#[test]
fn a_zero_rotation_leaves_the_frame_byte_identical() {
    let absent = render(&scene(None));
    let explicit_zero = render(&scene(Some(0.0)));
    assert_eq!(
        absent, explicit_zero,
        "writing `rotation: 0` changed the pixels, so every pre-existing scene did too"
    );
}

#[test]
fn a_quarter_turn_moves_the_marker_a_quarter_turn() {
    // The marker sits above the centre. Ninety degrees clockwise puts it to
    // the right of the centre — swapping which axis it is displaced along,
    // which no amount of translation or scaling can imitate.
    let up = centroid(&render(&scene(None)));
    let turned = centroid(&render(&scene(Some(90.0))));

    let cx = W as f32 / 2.0;
    let cy = H as f32 / 2.0;

    // Before: directly above centre.
    assert!(
        (up.0 - cx).abs() < 1.0,
        "marker is not on the vertical axis"
    );
    assert!(up.1 < cy - 35.0, "marker is not above the centre");

    // After: directly right of centre, the same distance out.
    assert!(
        (turned.1 - cy).abs() < 1.0,
        "after a quarter turn the marker is not on the horizontal axis: {turned:?}"
    );
    assert!(
        turned.0 > cx + 35.0,
        "after a quarter turn the marker is not to the right of the centre: {turned:?}"
    );

    let before = (up.0 - cx).hypot(up.1 - cy);
    let after = (turned.0 - cx).hypot(turned.1 - cy);
    assert!(
        (before - after).abs() < 1.0,
        "rotation changed the marker's distance from the centre ({before} -> {after}), \
         so it is not rotating about the canvas centre"
    );
}
