//! Draw order must be a property of the scene, not of how it is stored.
//!
//! This is the test that was missing. `sorted_root_ids` used a *stable* sort on
//! z-index, which preserved `HashMap` iteration order for ties — and that order
//! is randomised per process by `RandomState`. Two runs of the same export
//! drew tied objects in different sequences and produced different pixels
//! wherever they overlapped.
//!
//! Neither existing suite could see it. The golden-pixel tests and the
//! cross-backend parity suite both render inside a single process, where a
//! map's iteration order is fixed for its lifetime; the divergence only
//! appears *between* processes. So this asserts the property directly instead:
//! the same objects, inserted in different orders, must draw the same way.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use lumina_core::{SceneGraph, Timeline};
use lumina_renderer::{skia_backend::SkiaRenderer, Renderer};
use lumina_schema::Scene;

/// Four overlapping rectangles sharing one z-index, plus two that do not.
///
/// Overlap is the point: tied objects only reveal an ordering difference where
/// they cover each other.
fn tied_scene(order: &[&str]) -> Scene {
    let colours = [
        ("a", "#FF0000"),
        ("b", "#00FF00"),
        ("c", "#0000FF"),
        ("d", "#FFFF00"),
    ];
    let mut objects = serde_json::Map::new();
    for id in order {
        let (_, colour) = colours.iter().find(|(k, _)| k == id).expect("known id");
        objects.insert(
            (*id).to_string(),
            serde_json::json!({
                "type": "Rectangle",
                "properties": {
                    // All four cover the same area, so draw order decides the
                    // visible colour.
                    "x": 10.0, "y": 10.0, "width": 60.0, "height": 60.0,
                    "fill": colour, "z_index": 5, "opacity": 1.0
                }
            }),
        );
    }
    serde_json::from_value(serde_json::json!({
        "version": "1.0",
        "meta": { "title": "t", "author": "a", "created_at": "2026-01-01T00:00:00Z" },
        "canvas": {
            "width": 80, "height": 80, "fps": 30,
            "duration": 1.0, "background": "#000000"
        },
        "objects": objects,
        "timeline": []
    }))
    .expect("fixture scene")
}

fn render(scene: &Scene) -> Vec<u8> {
    let graph = SceneGraph::from_scene(scene);
    let states = Timeline::from_scene(scene).get_state_at(0.0);
    let mut r = SkiaRenderer::new();
    r.render_frame(
        &graph.objects,
        &states,
        scene.canvas.width,
        scene.canvas.height,
        &scene.canvas.background,
        None,
    )
    .expect("render")
}

#[test]
fn insertion_order_does_not_change_the_picture() {
    // Every permutation of four ids must render identically. If ordering falls
    // back to map iteration order, some of these differ.
    let orders: [&[&str]; 6] = [
        &["a", "b", "c", "d"],
        &["d", "c", "b", "a"],
        &["b", "d", "a", "c"],
        &["c", "a", "d", "b"],
        &["d", "a", "b", "c"],
        &["b", "a", "c", "d"],
    ];
    let reference = render(&tied_scene(orders[0]));
    for order in &orders[1..] {
        assert_eq!(
            render(&tied_scene(order)),
            reference,
            "inserting the same objects in the order {order:?} changed the rendered pixels — \
             draw order is depending on how the scene is stored rather than on what it contains"
        );
    }
}

#[test]
fn tied_objects_draw_in_id_order() {
    // Pins the specific rule, so the tie-break cannot be quietly changed to
    // something else that also happens to be stable.
    let scene = tied_scene(&["a", "b", "c", "d"]);
    let pixels = render(&scene);
    // The last id alphabetically wins the overlap: "d" is yellow.
    let idx = ((40 * 80) + 40) * 4;
    let px = &pixels[idx..idx + 4];
    assert_eq!(
        (px[0], px[1], px[2]),
        (255, 255, 0),
        "with equal z-index, the highest id should be drawn last; centre pixel was {px:?}"
    );
}

#[test]
fn z_index_still_wins_over_id() {
    // The tie-break must only apply to ties.
    let mut scene = tied_scene(&["a", "b", "c", "d"]);
    // Give "a" — first alphabetically — the highest z, so it must win.
    if let Some(lumina_schema::Object::Rectangle(p)) = scene.objects.get_mut("a") {
        p.z_index = 99;
    }
    let pixels = render(&scene);
    let idx = ((40 * 80) + 40) * 4;
    let px = &pixels[idx..idx + 4];
    assert_eq!(
        (px[0], px[1], px[2]),
        (255, 0, 0),
        "a higher z-index must beat the id tie-break; centre pixel was {px:?}"
    );
}

#[test]
fn group_children_order_is_also_total() {
    // Groups sort their children by the same rule, so a scene patch that
    // reorders a children list cannot change the picture.
    let build = |children: serde_json::Value| -> Scene {
        serde_json::from_value(serde_json::json!({
            "version": "1.0",
            "meta": { "title": "t", "author": "a", "created_at": "2026-01-01T00:00:00Z" },
            "canvas": {
                "width": 80, "height": 80, "fps": 30,
                "duration": 1.0, "background": "#000000"
            },
            "objects": {
                "g": { "type": "Group",
                       "properties": { "x": 0.0, "y": 0.0, "children": children } },
                "p": { "type": "Rectangle", "properties": {
                    "x": 10.0, "y": 10.0, "width": 60.0, "height": 60.0,
                    "fill": "#FF00FF", "z_index": 3, "opacity": 1.0 } },
                "q": { "type": "Rectangle", "properties": {
                    "x": 10.0, "y": 10.0, "width": 60.0, "height": 60.0,
                    "fill": "#00FFFF", "z_index": 3, "opacity": 1.0 } }
            },
            "timeline": []
        }))
        .expect("fixture scene")
    };
    assert_eq!(
        render(&build(serde_json::json!(["p", "q"]))),
        render(&build(serde_json::json!(["q", "p"]))),
        "reordering a group's children list changed the rendered pixels"
    );
}
