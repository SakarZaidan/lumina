//! Float, linear-light intermediates for a compositor (`AAA-OUT-07`).
//!
//! Three claims worth a test, and one worth *not* overclaiming:
//!
//! - the values are **linear**, not sRGB reinterpreted as float;
//! - alpha is **associated** (premultiplied), which is what `OpenEXR` specifies
//!   and what a compositor will assume regardless of what we intended;
//! - a fully opaque mid-grey round-trips to the linear value of that grey, so
//!   the transfer function is applied in the right direction — getting it
//!   backwards still produces a plausible-looking float image.
//!
//! What is *not* claimed: extra precision. The rasteriser has one pixel type,
//! 8-bit sRGB, so these floats carry 8-bit values converted exactly. The last
//! test pins that honestly rather than letting the format imply otherwise.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};

use lumina_renderer::skia_backend::SkiaRenderer;
use lumina_schema::Scene;

use crate::Exporter;

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lumina-exr-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// A flat scene of one colour, so every pixel has the same known answer.
fn flat_scene(background: &str, fill: Option<(&str, f64)>) -> Scene {
    let objects = match fill {
        Some((colour, opacity)) => serde_json::json!({
            "r": { "type": "Rectangle", "properties": {
                "x": 0, "y": 0, "width": 8, "height": 8,
                "fill": colour, "opacity": opacity, "z_index": 1 } }
        }),
        None => serde_json::json!({}),
    };
    serde_json::from_value(serde_json::json!({
        "version": "1.0",
        "meta": { "title": "exr", "author": "t", "created_at": "2026-09-03T00:00:00Z" },
        "canvas": { "width": 8, "height": 8, "fps": 10, "duration": 0.2,
                    "background": background },
        "objects": objects,
        "timeline": []
    }))
    .expect("scene")
}

fn first_pixel(dir: &Path) -> [f32; 4] {
    let img = image::open(dir.join("frame_0000.exr")).expect("frame written");
    let buf = img.to_rgba32f();
    let p = buf.get_pixel(4, 4).0;
    [p[0], p[1], p[2], p[3]]
}

fn export(scene: &Scene, name: &str) -> PathBuf {
    let dir = tmp(name);
    Exporter::new(SkiaRenderer::new())
        .export_exr_sequence(scene, &dir)
        .expect("exr export");
    dir
}

/// sRGB electro-optical transfer function, written out again here so the test
/// does not check the implementation against itself.
fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.040_45 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

#[test]
fn values_are_linear_light_not_srgb_in_a_float() {
    // Mid-grey is the case that separates the two: 0.5 in sRGB is 0.214 in
    // linear light. Writing the byte straight into a float would give 0.502,
    // which is a plausible number and the wrong one.
    let dir = export(&flat_scene("#808080", None), "grey");
    let px = first_pixel(&dir);

    let expected = srgb_to_linear(128.0 / 255.0);
    assert!(
        (px[0] - expected).abs() < 0.002,
        "mid-grey came out at {} — expected {expected:.4} (linear); \
         0.502 would mean the byte was copied without decoding",
        px[0]
    );
    assert_eq!(px[3], 1.0, "an opaque scene must be fully opaque");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_transfer_function_runs_in_the_right_direction() {
    // Encoding instead of decoding also produces a plausible image — every
    // value still lands in [0, 1] and the ordering is preserved. What
    // distinguishes them is the *side* of the identity: decoding darkens a
    // mid-tone, encoding brightens it.
    let dir = export(&flat_scene("#808080", None), "direction");
    let px = first_pixel(&dir);
    assert!(
        px[0] < 128.0 / 255.0,
        "mid-grey got brighter ({}), so the transfer function was applied backwards",
        px[0]
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn alpha_is_associated_as_openexr_specifies() {
    // Half-opaque pure red on a transparent canvas. Associated alpha means the
    // red channel carries the colour already multiplied by coverage, so it is
    // about *half* the linear value of full red — not the full value.
    let dir = export(
        &flat_scene("#00000000", Some(("#FF0000", 0.5))),
        "associated",
    );
    let px = first_pixel(&dir);

    assert!(
        (px[3] - 0.5).abs() < 0.01,
        "alpha should be about half, got {}",
        px[3]
    );
    assert!(
        px[0] < 0.75,
        "red is {} — at full linear intensity under half alpha, which is \
         straight alpha, and a compositor reading this as EXR will double it",
        px[0]
    );
    assert!(
        px[0] > 0.05,
        "red is {} — the colour was lost rather than premultiplied",
        px[0]
    );
    assert_eq!((px[1], px[2]), (0.0, 0.0), "red should stay pure");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_output_carries_exactly_the_precision_the_renderer_had() {
    // Not a limitation to hide: the rasteriser has one pixel type, 8-bit sRGB,
    // so every value in an EXR frame is one of 256 decoded levels. This pins
    // that, so nobody reads float channels as a claim of float rendering — and
    // so the test starts failing the day a deeper buffer lands, which is when
    // the docs need rewriting.
    let dir = export(&flat_scene("#3A7BD5", None), "precision");
    let px = first_pixel(&dir);

    for (i, v) in px[..3].iter().enumerate() {
        let nearest = (0..256)
            .map(|b| srgb_to_linear(b as f32 / 255.0))
            .min_by(|a, b| (a - v).abs().partial_cmp(&(b - v).abs()).expect("finite"))
            .expect("non-empty");
        assert!(
            (nearest - v).abs() < 1e-4,
            "channel {i} is {v}, which is not one of the 256 levels an 8-bit \
             source can produce — the renderer gained precision, so update the \
             docs on export_exr_sequence"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_frame_is_written_per_frame_of_the_scene() {
    let scene = flat_scene("#101018", None);
    let dir = export(&scene, "count");
    let expected = (scene.canvas.duration * scene.canvas.fps as f32).ceil() as usize;
    let written = std::fs::read_dir(&dir)
        .expect("dir")
        .filter(|e| {
            e.as_ref()
                .map(|e| e.path().extension().is_some_and(|x| x == "exr"))
                .unwrap_or(false)
        })
        .count();
    assert_eq!(written, expected);
    let _ = std::fs::remove_dir_all(&dir);
}
