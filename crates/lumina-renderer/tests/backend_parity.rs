//! Cross-backend pixel-diff harness (TD-11).
//!
//! Renders each fixture scene in `tests/fixtures/` on both the Skia (CPU)
//! and Vello (GPU, CPU-fallback adapter) backends at several timeline
//! sample points and asserts the outputs agree within a per-fixture
//! tolerance. On failure both frames plus a per-pixel heat map are written
//! to `target/parity-failures/<fixture>_t<time>/` (override the directory
//! with `LUMINA_PARITY_OUT`) so divergence can be inspected by eye.
//!
//! Vello needs a wgpu fallback adapter (e.g. Mesa lavapipe). When the
//! adapter is unavailable the suite skips with a note — unless
//! `LUMINA_REQUIRE_VELLO=1` is set (as in CI), in which case the absence
//! is a hard failure so parity can never silently stop being checked.
//!
//! Fixtures deliberately cover one feature each; scenes for gradients,
//! shadows, rounded rectangles and dashes are added as the corresponding
//! Vello features land (TD-01).

// Integration tests are not `#[cfg(test)]` items, so `allow-unwrap-in-tests`
// in clippy.toml does not reach them. Panicking on setup failure is correct
// here: a fixture that cannot be loaded must fail the test loudly, not be
// silently skipped.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use lumina_core::{SceneGraph, Timeline};
use lumina_renderer::skia_backend::SkiaRenderer;
use lumina_renderer::vello_backend::VelloRenderer;
use lumina_renderer::Renderer;
use lumina_schema::Scene;
use std::path::{Path, PathBuf};

/// Per-fixture comparison budget.
///
/// CPU (tiny-skia) and GPU (vello) rasterizers legitimately disagree on
/// anti-aliased edge coverage, so a raw per-pixel delta would drown real
/// bugs in AA noise. A pixel therefore only counts as *differing* when the
/// disagreement cannot be explained by a sub-pixel edge shift: each pixel
/// must find a matching pixel (within `max_channel_delta`) inside the 3×3
/// neighborhood of the same location in the other image, in both
/// directions. Thin features that exist on one backend only fail this
/// rescue and are counted in full.
#[derive(Clone, Copy)]
struct Tolerance {
    /// Channel delta treated as identical on direct comparison.
    max_channel_delta: u8,
    /// Channel delta accepted when matching against the 3×3 neighborhood —
    /// looser than the direct tolerance because AA coverage on curved edges
    /// rounds differently per rasterizer. Systematic errors that this would
    /// forgive are caught by `max_mean_delta`.
    aa_tol: u8,
    /// Fraction of unexplained pixels allowed.
    max_diff_pixel_frac: f64,
    /// Mean absolute per-channel difference allowed over the whole frame —
    /// catches systematic shifts (blend-space/gamma tints) that per-pixel
    /// neighborhood matching would forgive.
    max_mean_delta: f64,
}

const DEFAULT_TOL: Tolerance = Tolerance {
    max_channel_delta: 8,
    aa_tol: 32,
    max_diff_pixel_frac: 0.001,
    max_mean_delta: 0.6,
};

/// Cubic curves tessellate slightly differently per rasterizer, so path
/// edge pixels need more AA headroom than analytic shapes.
const PATH_TOL: Tolerance = Tolerance {
    max_channel_delta: 8,
    aa_tol: 48,
    max_diff_pixel_frac: 0.001,
    max_mean_delta: 0.6,
};

/// Gradient ramps may interpolate slightly differently between rasterizers
/// (mid-ramp rounding), so the direct per-channel tolerance is wider while
/// the pixel and mean budgets stay strict.
const GRADIENT_TOL: Tolerance = Tolerance {
    max_channel_delta: 8,
    aa_tol: 48,
    max_diff_pixel_frac: 0.001,
    max_mean_delta: 0.8,
};

/// Wider budget: text renders through two parallel code paths — Skia
/// composites each glyph directly, Vello resamples a pre-rendered string
/// bitmap (`raster.rs`) — which shifts glyph AA and low-opacity blending.
/// Unifying the two paths is tracked as TD-18; tighten this when it lands.
const TEXT_TOL: Tolerance = Tolerance {
    max_channel_delta: 12,
    aa_tol: 48,
    max_diff_pixel_frac: 0.015,
    max_mean_delta: 1.5,
};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crate lives two levels below the workspace root")
        .to_path_buf()
}

fn load_fixture(name: &str) -> Scene {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(format!("{name}.lsf"));
    let json = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read fixture {path:?}: {e}"));
    serde_json::from_str(&json).unwrap_or_else(|e| panic!("fixture {name} is not valid LSF: {e}"))
}

/// Load scene font and image/SVG assets into a renderer, resolving
/// repo-relative paths (the convention used by `examples/`) against the
/// workspace root.
fn load_assets(renderer: &mut dyn Renderer, scene: &Scene) {
    for font in &scene.assets.fonts {
        let path = workspace_root().join(&font.path);
        let data =
            std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read font {path:?}: {e}"));
        renderer
            .load_font(&font.id, &data)
            .expect("font load failed");
    }
    for image in &scene.assets.images {
        let path = workspace_root().join(&image.path);
        let data =
            std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read image {path:?}: {e}"));
        renderer
            .load_image(&image.id, &data)
            .expect("image load failed");
    }
}

fn render_at(renderer: &mut dyn Renderer, scene: &Scene, t: f32) -> Vec<u8> {
    let graph = SceneGraph::from_scene(scene);
    let timeline = Timeline::from_scene(scene);
    let states = timeline.get_state_at(t);
    let camera = timeline.get_camera_at(t, scene);
    renderer.set_time(t);
    renderer
        .render_frame(
            &graph.objects,
            &states,
            scene.canvas.width,
            scene.canvas.height,
            &scene.canvas.background,
            Some(&camera),
        )
        .expect("render_frame failed")
}

fn vello_or_skip() -> Option<VelloRenderer> {
    match VelloRenderer::new() {
        Ok(r) => Some(r),
        Err(e) => {
            if std::env::var("LUMINA_REQUIRE_VELLO").as_deref() == Ok("1") {
                panic!("LUMINA_REQUIRE_VELLO=1 but no wgpu adapter is available: {e}");
            }
            eprintln!("skipping parity test: no wgpu adapter ({e})");
            None
        }
    }
}

struct DiffStats {
    differing: usize,
    max_delta: u8,
    mean_delta: f64,
    total: usize,
}

fn channel_delta(a: &[u8], b: &[u8]) -> u8 {
    (0..3).map(|c| a[c].abs_diff(b[c])).max().unwrap()
}

/// Does the pixel at (x, y) in `from` match any pixel in the 3×3
/// neighborhood of (x, y) in `to`, within `tol` per channel?
fn neighborhood_match(from: &[u8], to: &[u8], x: u32, y: u32, w: u32, h: u32, tol: u8) -> bool {
    let p = ((y * w + x) * 4) as usize;
    for dy in -1i64..=1 {
        for dx in -1i64..=1 {
            let (nx, ny) = (x as i64 + dx, y as i64 + dy);
            if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                continue;
            }
            let q = ((ny as u32 * w + nx as u32) * 4) as usize;
            if channel_delta(&from[p..p + 4], &to[q..q + 4]) <= tol {
                return true;
            }
        }
    }
    false
}

fn diff(skia: &[u8], vello: &[u8], w: u32, h: u32, tol: Tolerance) -> (DiffStats, Vec<u8>) {
    assert_eq!(skia.len(), vello.len(), "frame byte lengths differ");
    let total = skia.len() / 4;
    let mut differing = 0usize;
    let mut max_delta = 0u8;
    let mut sum_delta = 0u64;
    let mut heat = vec![0u8; skia.len()];
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            let (s, v) = (&skia[i..i + 4], &vello[i..i + 4]);
            // Fixtures use opaque backgrounds so premultiplied (Skia) and
            // straight (Vello) alpha coincide; enforce that assumption.
            assert_eq!(s[3], 255, "Skia produced translucent pixel at ({x},{y})");
            assert_eq!(v[3], 255, "Vello produced translucent pixel at ({x},{y})");
            let d = channel_delta(s, v);
            max_delta = max_delta.max(d);
            sum_delta += (0..3).map(|c| s[c].abs_diff(v[c]) as u64).sum::<u64>();
            let explained = d <= tol.max_channel_delta
                || (neighborhood_match(skia, vello, x, y, w, h, tol.aa_tol)
                    && neighborhood_match(vello, skia, x, y, w, h, tol.aa_tol));
            if !explained {
                differing += 1;
            }
            let px = &mut heat[i..i + 4];
            px[0] = d.saturating_mul(8);
            px[1] = if explained { 0 } else { 255 };
            px[3] = 255;
        }
    }
    (
        DiffStats {
            differing,
            max_delta,
            mean_delta: sum_delta as f64 / (total as f64 * 3.0),
            total,
        },
        heat,
    )
}

fn dump_failure(
    name: &str,
    t: f32,
    w: u32,
    h: u32,
    skia: &[u8],
    vello: &[u8],
    heat: &[u8],
) -> PathBuf {
    let base = std::env::var("LUMINA_PARITY_OUT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| workspace_root().join("target/parity-failures"));
    let dir = base.join(format!("{name}_t{t:.2}"));
    std::fs::create_dir_all(&dir).expect("cannot create parity-failure dir");
    for (file, data) in [("skia.png", skia), ("vello.png", vello), ("diff.png", heat)] {
        image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(w, h, data.to_vec())
            .expect("buffer size mismatch")
            .save(dir.join(file))
            .expect("cannot write failure artifact");
    }
    dir
}

/// Render `name` on both backends at t = 0, mid, and end of the scene
/// duration, asserting agreement within `tol` at every sample.
fn assert_parity(name: &str, tol: Tolerance) {
    let Some(mut vello) = vello_or_skip() else {
        return;
    };
    let mut skia = SkiaRenderer::new();
    let scene = load_fixture(name);
    load_assets(&mut skia, &scene);
    load_assets(&mut vello, &scene);

    let dur = scene.canvas.duration;
    let (w, h) = (scene.canvas.width, scene.canvas.height);
    for t in [0.0, dur * 0.5, dur] {
        let frame_skia = render_at(&mut skia, &scene, t);
        let frame_vello = render_at(&mut vello, &scene, t);
        let (stats, heat) = diff(&frame_skia, &frame_vello, w, h, tol);
        let frac = stats.differing as f64 / stats.total as f64;
        if frac > tol.max_diff_pixel_frac || stats.mean_delta > tol.max_mean_delta {
            let dir = dump_failure(name, t, w, h, &frame_skia, &frame_vello, &heat);
            panic!(
                "parity failure in '{name}' at t={t}: {}/{} pixels ({:.4}%) unexplained \
                 by AA (allowed {:.4}%), mean channel delta {:.3} (allowed {:.3}), \
                 max delta {}; artifacts in {dir:?}",
                stats.differing,
                stats.total,
                frac * 100.0,
                tol.max_diff_pixel_frac * 100.0,
                stats.mean_delta,
                tol.max_mean_delta,
                stats.max_delta,
            );
        }
    }
}

#[test]
fn parity_01_shapes_solid() {
    assert_parity("01_shapes_solid", DEFAULT_TOL);
}

#[test]
fn parity_02_polygon_arrow() {
    assert_parity("02_polygon_arrow", DEFAULT_TOL);
}

#[test]
fn parity_03_svg_path() {
    assert_parity("03_svg_path", PATH_TOL);
}

#[test]
fn parity_04_text() {
    assert_parity("04_text", TEXT_TOL);
}

#[test]
fn parity_05_gradient_linear() {
    assert_parity("05_gradient_linear", GRADIENT_TOL);
}

#[test]
fn parity_06_gradient_radial() {
    assert_parity("06_gradient_radial", GRADIENT_TOL);
}

#[test]
fn parity_07_shadows() {
    assert_parity("07_shadows", DEFAULT_TOL);
}

#[test]
fn parity_08_rounded_rect() {
    assert_parity("08_rounded_rect", DEFAULT_TOL);
}

#[test]
fn parity_09_dash_fraction() {
    assert_parity("09_dash_fraction", DEFAULT_TOL);
}

#[test]
fn parity_10_groups() {
    assert_parity("10_groups", DEFAULT_TOL);
}

#[test]
fn parity_11_camera() {
    assert_parity("11_camera", DEFAULT_TOL);
}

#[test]
fn parity_12_particles() {
    assert_parity("12_particles", DEFAULT_TOL);
}

#[test]
fn parity_13_opacity_zindex() {
    assert_parity("13_opacity_zindex", DEFAULT_TOL);
}

#[test]
fn parity_14_plot_axes() {
    assert_parity("14_plot_axes", DEFAULT_TOL);
}

#[test]
fn parity_16_svg_asset() {
    assert_parity("16_svg_asset", DEFAULT_TOL);
}

#[test]
fn parity_17_showcase_combined() {
    // Combines gradients, shadows, rounded rects, text, groups, particles
    // and a camera move; text is present, so use the text budget.
    assert_parity("17_showcase_combined", TEXT_TOL);
}

// ── Behavioural parity ──────────────────────────────────────────────────────
//
// The pixel suite above compares frames that *both* backends produced. When
// one backend errors and the other silently skips the object, there is no pair
// of frames to compare and the divergence is invisible to it — which is how
// the Arrow case survived: Skia aborted the whole export, Vello rendered the
// frame without the object, and the same scene gave different output depending
// on `--backend`.
//
// These tests assert the backends agree on *failure*, not on pixels.

/// A scene whose Arrow is well-formed until a timeline keyframe overwrites
/// `from` with a one-element array.
///
/// `ArrowProps.from` is `[f32; 2]`, so serde guarantees two elements at parse
/// time. Timeline state is untyped `serde_json::Value` (TD-07), so a keyframe
/// is the one route by which malformed geometry reaches a renderer.
fn malformed_arrow_state() -> (
    std::collections::HashMap<String, lumina_schema::Object>,
    std::collections::HashMap<String, serde_json::Value>,
) {
    let scene: Scene = serde_json::from_value(serde_json::json!({
        "version": "1.0",
        "meta": { "title": "t", "author": "a", "created_at": "2026-01-01T00:00:00Z" },
        "canvas": { "width": 64, "height": 64, "fps": 30, "duration": 1.0, "background": "#000000" },
        "objects": {
            "a": { "type": "Arrow", "properties": { "from": [8.0, 8.0], "to": [56.0, 56.0] } }
        },
        "timeline": []
    }))
    .expect("fixture scene must deserialise");

    let graph = SceneGraph::from_scene(&scene);
    let mut states = Timeline::from_scene(&scene).get_state_at(0.0);
    // What a bad generator, or a hand-edited keyframe, produces.
    if let Some(state) = states.get_mut("a") {
        state["from"] = serde_json::json!([8.0]);
    }
    (graph.objects, states)
}

#[test]
fn both_backends_reject_a_malformed_arrow() {
    let (objects, states) = malformed_arrow_state();

    let mut skia = SkiaRenderer::new();
    let skia_result = skia.render_frame(&objects, &states, 64, 64, "#000000", None);
    assert!(
        skia_result.is_err(),
        "the CPU backend must reject a malformed Arrow rather than drawing something"
    );

    let Some(mut vello) = vello_or_skip() else {
        return;
    };
    let vello_result = vello.render_frame(&objects, &states, 64, 64, "#000000", None);
    assert!(
        vello_result.is_err(),
        "the GPU backend must reject the same input the CPU backend rejects. Silently skipping \
         the object makes the render depend on --backend, and the pixel suite cannot catch it."
    );
}

#[test]
fn both_backends_render_a_well_formed_arrow() {
    // The guard above must not reject valid input on either backend.
    let scene: Scene = serde_json::from_value(serde_json::json!({
        "version": "1.0",
        "meta": { "title": "t", "author": "a", "created_at": "2026-01-01T00:00:00Z" },
        "canvas": { "width": 64, "height": 64, "fps": 30, "duration": 1.0, "background": "#000000" },
        "objects": {
            "a": { "type": "Arrow", "properties": { "from": [8.0, 8.0], "to": [56.0, 56.0] } }
        },
        "timeline": []
    }))
    .expect("fixture scene must deserialise");
    let objects = SceneGraph::from_scene(&scene).objects;
    let states = Timeline::from_scene(&scene).get_state_at(0.0);

    let mut skia = SkiaRenderer::new();
    assert!(skia
        .render_frame(&objects, &states, 64, 64, "#000000", None)
        .is_ok());

    if let Some(mut vello) = vello_or_skip() {
        assert!(vello
            .render_frame(&objects, &states, 64, 64, "#000000", None)
            .is_ok());
    }
}
