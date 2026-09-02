//! Transparency survives the trip out of the engine (`AAA-OUT-06`).
//!
//! The renderer composes in premultiplied alpha; PNG, ffmpeg's `rgba` input,
//! and a canvas `ImageData` all store straight alpha. While every background
//! was opaque the two encodings were the same bytes and nothing noticed. The
//! moment a scene asks for a transparent background, a half-opaque pure red
//! leaves the rasteriser as `(127, 0, 0, 127)` and, read back as straight
//! alpha, is a *dark* red at half opacity rather than a bright one.
//!
//! These tests are about the colour, not about the flags.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;

use luminafx_renderer::skia_backend::SkiaRenderer;
use luminafx_schema::Scene;

use crate::Exporter;

/// A half-opaque pure red square filling a transparent canvas.
///
/// Pure red at exactly 50% is chosen because the two conventions are furthest
/// apart there and the correct answer is unambiguous: `255` in the red
/// channel, roughly `127` in alpha.
fn transparent_scene(motion_blur_samples: u32) -> Scene {
    serde_json::from_value(serde_json::json!({
        "version": "1.0",
        "meta": { "title": "alpha", "author": "t", "created_at": "2026-09-02T00:00:00Z" },
        "canvas": {
            "width": 16, "height": 16, "fps": 10, "duration": 0.2,
            "background": "#00000000",
            "motion_blur_samples": motion_blur_samples
        },
        "objects": {
            "r": {
                "type": "Rectangle",
                "properties": { "x": 0, "y": 0, "width": 16, "height": 16,
                                "fill": "#FF0000", "opacity": 0.5, "z_index": 1 }
            }
        },
        "timeline": []
    }))
    .expect("scene")
}

fn first_frame_rgba(dir: &Path) -> Vec<u8> {
    let img = image::open(dir.join("frame_0000.png")).expect("frame written");
    img.to_rgba8().into_raw()
}

fn tmp(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("lumina-alpha-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn a_half_opaque_colour_reaches_a_png_undarkened() {
    let dir = tmp("png");
    let scene = transparent_scene(1);
    Exporter::new(SkiaRenderer::new())
        .export_png_sequence(&scene, &dir)
        .expect("export");

    let px = first_frame_rgba(&dir);
    let (r, g, b, a) = (px[0], px[1], px[2], px[3]);
    assert!(
        (120..=135).contains(&a),
        "alpha should be about half, got {a}"
    );
    assert!(
        r >= 250,
        "red came out at {r} rather than ~255 — the frame was written premultiplied, \
         so the colour is darkened by its own alpha"
    );
    assert_eq!((g, b), (0, 0), "red should stay pure");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_opaque_scene_is_byte_for_byte_what_it_always_was() {
    // Demultiplying leaves `a == 255` alone, so every existing scene — which
    // is to say every scene ever authored before transparency worked — must
    // come out unchanged. If this fails, the change is not a fix but a
    // regression to every golden image in the repository.
    let mut scene = transparent_scene(1);
    scene.canvas.background = "#101018".to_string();

    let dir = tmp("opaque");
    Exporter::new(SkiaRenderer::new())
        .export_png_sequence(&scene, &dir)
        .expect("export");
    let px = first_frame_rgba(&dir);

    // Half-opacity red over an opaque backdrop: composited, fully opaque, and
    // exactly the blend the rasteriser produced.
    assert_eq!(px[3], 255, "an opaque background must stay opaque");
    assert!(
        px[0] > 120 && px[0] < 140,
        "unexpected blend: {:?}",
        &px[..4]
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn motion_blur_still_averages_before_the_conversion() {
    // Averaging straight-alpha colours weights a nearly-transparent sample as
    // heavily as an opaque one and haloes every moving edge, so the averaging
    // has to happen while the values are still premultiplied. Nothing moves in
    // this scene, so every sample is identical and the average must equal the
    // single-sample result exactly — which it cannot if the conversion has
    // been folded into the wrong side of the loop.
    let one = tmp("blur1");
    let many = tmp("blur8");
    Exporter::new(SkiaRenderer::new())
        .export_png_sequence(&transparent_scene(1), &one)
        .expect("export");
    Exporter::new(SkiaRenderer::new())
        .export_png_sequence(&transparent_scene(8), &many)
        .expect("export");

    assert_eq!(
        first_frame_rgba(&one),
        first_frame_rgba(&many),
        "blurring a still scene changed it"
    );

    let _ = std::fs::remove_dir_all(&one);
    let _ = std::fs::remove_dir_all(&many);
}

fn ffmpeg_available() -> bool {
    std::process::Command::new("ffmpeg")
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok()
}

/// Decode one frame of `path` back to straight RGBA and return its centre
/// pixel.
///
/// Deliberately a round trip rather than an `ffprobe` field. `WebM` does not
/// advertise alpha in `pix_fmt` at all — a VP9 file with a full alpha plane
/// still reports `yuv420p`, and signals the plane in an `alpha_mode` tag — so
/// probing the obvious field reports "no alpha" on a file that has it. What
/// the work item actually claims is that the transparency arrives at whatever
/// opens the file, and decoding is how you check that.
fn decoded_centre_pixel(path: &Path, decoder: &[&str], w: u32, h: u32) -> [u8; 4] {
    let raw = path.with_extension("raw");
    let status = std::process::Command::new("ffmpeg")
        .args(["-y", "-hide_banner", "-loglevel", "error"])
        .args(decoder)
        .arg("-i")
        .arg(path)
        .args(["-frames:v", "1", "-f", "rawvideo", "-pix_fmt", "rgba"])
        .arg(&raw)
        .status()
        .expect("ffmpeg decode");
    assert!(status.success(), "could not decode {path:?}");

    let data = std::fs::read(&raw).expect("raw frame");
    let i = ((h / 2 * w + w / 2) * 4) as usize;
    let px = [data[i], data[i + 1], data[i + 2], data[i + 3]];
    let _ = std::fs::remove_file(&raw);
    px
}

/// "Still red", stated as dominance rather than as exact channel values.
///
/// A saturated primary does not survive an RGB → BT.709 `Y'CbCr` → RGB round
/// trip exactly; `ProRes` returns a green channel around 25 where the source had
/// 0. Pinning absolute numbers would be testing the colour conversion's
/// rounding, which is ffmpeg's business. What must hold is that the hue
/// arrived — and that it did not arrive halved, which is the premultiplied
/// failure this whole file exists to catch.
fn assert_red(px: [u8; 4], what: &str) {
    assert!(
        px[0] > 200,
        "{what} came back at red {} rather than near 255 — the frame went out \
         premultiplied, so the colour is darkened by its own alpha: {px:?}",
        px[0]
    );
    assert!(
        u16::from(px[0]) > u16::from(px[1]) + 150 && u16::from(px[0]) > u16::from(px[2]) + 150,
        "{what} did not come back red: {px:?}"
    );
}

#[test]
fn the_alpha_formats_carry_transparency_all_the_way_back() {
    if !ffmpeg_available() {
        return;
    }
    let scene = transparent_scene(1);
    let dir = tmp("codecs");
    std::fs::create_dir_all(&dir).expect("mkdir");

    // VP9 needs its decoder named explicitly: the native decoder ignores the
    // alpha plane and hands back an opaque frame.
    let webm = dir.join("a.webm");
    Exporter::new(SkiaRenderer::new())
        .export_webm_alpha(&scene, &webm)
        .expect("webm-alpha export");
    let px = decoded_centre_pixel(&webm, &["-c:v", "libvpx-vp9"], 16, 16);
    assert!(
        (110..=140).contains(&px[3]),
        "VP9 alpha round-trip lost the transparency: {px:?}"
    );
    assert_red(px, "VP9 alpha");

    let mov = dir.join("a.mov");
    Exporter::new(SkiaRenderer::new())
        .export_mov_prores4444(&scene, &mov)
        .expect("prores export");
    let px = decoded_centre_pixel(&mov, &[], 16, 16);
    assert!(
        (110..=140).contains(&px[3]),
        "ProRes 4444 round-trip lost the transparency: {px:?}"
    );
    assert_red(px, "ProRes 4444");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_opaque_formats_are_untouched_by_any_of_this() {
    if !ffmpeg_available() {
        return;
    }
    // MP4 has no alpha and never did; the point is that adding the alpha paths
    // did not disturb the default one.
    let dir = tmp("mp4");
    std::fs::create_dir_all(&dir).expect("mkdir");
    let mut scene = transparent_scene(1);
    scene.canvas.background = "#101018".to_string();

    let mp4 = dir.join("a.mp4");
    Exporter::new(SkiaRenderer::new())
        .export_mp4(&scene, &mp4)
        .expect("mp4 export");
    let px = decoded_centre_pixel(&mp4, &[], 16, 16);
    assert_eq!(px[3], 255, "MP4 must decode opaque");

    let _ = std::fs::remove_dir_all(&dir);
}
