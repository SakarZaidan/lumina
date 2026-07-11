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

/// Load scene font assets into a renderer, resolving repo-relative paths
/// (the convention used by `examples/`) against the workspace root.
fn load_assets(renderer: &mut dyn Renderer, scene: &Scene) {
    for font in &scene.assets.fonts {
        let path = workspace_root().join(&font.path);
        let data =
            std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read font {path:?}: {e}"));
        renderer
            .load_font(&font.id, &data)
            .expect("font load failed");
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
