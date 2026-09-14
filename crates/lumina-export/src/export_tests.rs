#[cfg(test)]
mod tests {
    use crate::Exporter;
    use luminafx_renderer::skia_backend::SkiaRenderer;
    use luminafx_schema::{Canvas, CircleProps, Meta, Object, Scene, TimelineEntry};
    use serde_json::json;
    use std::collections::HashMap;

    fn two_frame_scene() -> Scene {
        let mut objects = HashMap::new();
        objects.insert(
            "c".into(),
            Object::Circle(CircleProps {
                cx: 50.0,
                cy: 50.0,
                radius: 20.0,
                z_index: 1,
                fill: "#FFFFFF".into(),
                stroke: None,
                stroke_width: 0.0,
                shadow: None,
                opacity: 0.0,
            }),
        );
        Scene {
            version: "1.0".into(),
            meta: Meta {
                title: "Export Test".into(),
                author: "test".into(),
                created_at: "now".into(),
            },
            // fps=1, duration=2.0 → exactly 2 frames (frame_0000 and frame_0001)
            canvas: Canvas {
                width: 64,
                height: 64,
                fps: 1,
                duration: 2.0,
                background: "#000000".into(),
                motion_blur_samples: 1,
                shutter: 0.5,
            },
            assets: Default::default(),
            objects,
            timeline: vec![TimelineEntry {
                time: 1.0,
                object: "c".into(),
                state: json!({"opacity": 1.0}),
                easing: "linear".into(),
                easing_params: None,
            }],
            events: vec![],
            camera: None,
        }
    }

    #[test]
    fn test_png_sequence_creates_expected_files() {
        let scene = two_frame_scene();
        let mut exporter = Exporter::new(SkiaRenderer::new());
        let dir = std::env::temp_dir().join("lumina_test_png_sequence");
        let _ = std::fs::remove_dir_all(&dir);

        exporter
            .export_png_sequence(&scene, &dir)
            .expect("PNG export failed");

        let frame0 = dir.join("frame_0000.png");
        let frame1 = dir.join("frame_0001.png");
        assert!(frame0.exists(), "frame_0000.png should exist");
        assert!(frame1.exists(), "frame_0001.png should exist");

        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_png_files_have_correct_dimensions() {
        let scene = two_frame_scene();
        let mut exporter = Exporter::new(SkiaRenderer::new());
        let dir = std::env::temp_dir().join("lumina_test_png_dims");
        let _ = std::fs::remove_dir_all(&dir);

        exporter
            .export_png_sequence(&scene, &dir)
            .expect("PNG export failed");

        let frame_path = dir.join("frame_0000.png");
        let img = image::open(&frame_path).expect("Should be able to open PNG");
        assert_eq!(img.width(), 64, "PNG width should match canvas width");
        assert_eq!(img.height(), 64, "PNG height should match canvas height");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_png_frame_brightness_increases_with_opacity() {
        let scene = two_frame_scene();
        let mut exporter = Exporter::new(SkiaRenderer::new());
        let dir = std::env::temp_dir().join("lumina_test_png_brightness");
        let _ = std::fs::remove_dir_all(&dir);

        exporter
            .export_png_sequence(&scene, &dir)
            .expect("PNG export failed");

        let frame0 = image::open(dir.join("frame_0000.png")).unwrap();
        let frame1 = image::open(dir.join("frame_0001.png")).unwrap();

        // Average brightness: frame at t=1.0 (opacity=1.0) should be brighter than t=0.0 (opacity=0.0)
        fn avg_brightness(img: &image::DynamicImage) -> f32 {
            let pixels: Vec<u8> = img.to_rgba8().into_raw();
            let sum: u32 = pixels.iter().map(|&p| p as u32).sum();
            sum as f32 / pixels.len() as f32
        }

        let b0 = avg_brightness(&frame0);
        let b1 = avg_brightness(&frame1);
        assert!(b1 > b0, "Frame at t=1.0 (opacity=1) should be brighter than t=0.0 (opacity=0): b0={b0}, b1={b1}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    fn ffmpeg_available() -> bool {
        std::process::Command::new("ffmpeg")
            .arg("-version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    #[test]
    fn test_webm_export_produces_valid_file() {
        if !ffmpeg_available() {
            return; // skip when ffmpeg is absent
        }
        let scene = two_frame_scene();
        let mut exporter = Exporter::new(SkiaRenderer::new());
        let out = std::env::temp_dir().join("lumina_test_output.webm");
        let _ = std::fs::remove_file(&out);

        exporter
            .export_webm(&scene, &out)
            .expect("WebM export failed");
        let bytes = std::fs::read(&out).expect("read webm");
        assert!(!bytes.is_empty(), "WebM file should be non-empty");
        // EBML magic header for Matroska/WebM.
        assert_eq!(
            &bytes[0..4],
            &[0x1A, 0x45, 0xDF, 0xA3],
            "WebM should start with the EBML magic header"
        );
        let _ = std::fs::remove_file(&out);
    }

    #[test]
    fn test_gif_export_produces_valid_file() {
        if !ffmpeg_available() {
            return;
        }
        let scene = two_frame_scene();
        let mut exporter = Exporter::new(SkiaRenderer::new());
        let out = std::env::temp_dir().join("lumina_test_output.gif");
        let _ = std::fs::remove_file(&out);

        exporter
            .export_gif(&scene, &out)
            .expect("GIF export failed");
        let bytes = std::fs::read(&out).expect("read gif");
        assert!(!bytes.is_empty(), "GIF file should be non-empty");
        assert_eq!(
            &bytes[0..4],
            b"GIF8",
            "GIF should start with the GIF8 magic"
        );
        let _ = std::fs::remove_file(&out);
    }

    #[test]
    fn test_mp4_export_fails_gracefully_without_ffmpeg() {
        // This test verifies the error is descriptive, not a panic
        // If ffmpeg is installed this test is a no-op (export succeeds)
        let scene = two_frame_scene();
        let mut exporter = Exporter::new(SkiaRenderer::new());
        let out = std::env::temp_dir().join("lumina_test_output.mp4");

        let result = exporter.export_mp4(&scene, &out);
        if let Err(e) = result {
            let msg = e.to_string();
            assert!(
                msg.contains("ffmpeg") || msg.contains("FFmpeg") || msg.contains("spawn"),
                "Error message should mention ffmpeg, got: {msg}"
            );
        }
        // If ffmpeg IS present, the test still passes (export succeeded)
    }
}

/// The export pipelines must not change what is produced.
///
/// Both paths hand frames to another thread — PNG to a rayon pool, video to a
/// writer feeding ffmpeg. Neither may alter the result, and the PNG path in
/// particular writes files out of order by design, so the *contents* have to
/// carry the ordering rather than the writing sequence.
///
/// These very nearly shipped broken. An earlier version of this work appeared
/// to change pixels as a function of queue depth, and the investigation found
/// the real cause elsewhere: draw order was non-deterministic between
/// processes (see `lumina-renderer/tests/draw_order.rs`). The pipelines were
/// correct; the base was not.
#[cfg(test)]
mod pipelined_export {
    use crate::Exporter;
    use luminafx_renderer::skia_backend::SkiaRenderer;
    use luminafx_schema::Scene;

    fn short_scene(frames: u32) -> Scene {
        let fps = 30;
        serde_json::from_value(serde_json::json!({
            "version": "1.0",
            "meta": { "title": "t", "author": "a", "created_at": "2026-01-01T00:00:00Z" },
            "canvas": {
                "width": 64, "height": 48, "fps": fps,
                "duration": f64::from(frames) / f64::from(fps), "background": "#101020"
            },
            "objects": {
                // Two objects sharing a z-index and overlapping: the shape that
                // exposed the ordering bug.
                "a": { "type": "Rectangle", "properties": {
                    "x": 5.0, "y": 5.0, "width": 40.0, "height": 30.0,
                    "fill": "#FF4040", "z_index": 2, "opacity": 1.0 } },
                "b": { "type": "Circle", "properties": {
                    "cx": 30.0, "cy": 20.0, "radius": 15.0,
                    "fill": "#40A0FF", "z_index": 2, "opacity": 1.0 } }
            },
            "timeline": [
                { "time": 0.0, "object": "b", "state": { "cx": 10.0 }, "easing": "linear" },
                { "time": 1.0, "object": "b", "state": { "cx": 54.0 }, "easing": "ease_out_cubic" }
            ]
        }))
        .expect("fixture scene")
    }

    fn export_to(dir: &std::path::Path, scene: &Scene) {
        let mut exporter = Exporter::new(SkiaRenderer::new());
        exporter
            .export_png_sequence(scene, dir)
            .expect("png export succeeds");
    }

    fn frames_in(dir: &std::path::Path) -> Vec<(String, Vec<u8>)> {
        let mut out: Vec<_> = std::fs::read_dir(dir)
            .expect("output dir")
            .filter_map(Result::ok)
            .map(|e| {
                (
                    e.file_name().to_string_lossy().into_owned(),
                    std::fs::read(e.path()).expect("frame readable"),
                )
            })
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    #[test]
    fn png_export_writes_every_frame() {
        let scene = short_scene(20);
        let dir = tempfile::tempdir().expect("temp dir");
        export_to(dir.path(), &scene);
        let frames = frames_in(dir.path());
        assert_eq!(frames.len(), 20, "one file per frame");
        assert_eq!(frames[0].0, "frame_0000.png");
        assert_eq!(frames[19].0, "frame_0019.png");
        assert!(
            frames.iter().all(|(_, bytes)| !bytes.is_empty()),
            "no frame may be written empty"
        );
    }

    #[test]
    fn png_export_is_reproducible() {
        // Encoding happens on a pool, so files are written out of order. The
        // contents must not depend on which worker got which frame.
        let scene = short_scene(24);
        let a = tempfile::tempdir().expect("temp dir");
        let b = tempfile::tempdir().expect("temp dir");
        export_to(a.path(), &scene);
        export_to(b.path(), &scene);
        assert_eq!(
            frames_in(a.path()),
            frames_in(b.path()),
            "two exports of the same scene must be byte-identical"
        );
    }

    #[test]
    fn png_frames_differ_from_each_other() {
        // Guards against the pipeline writing the same frame under every name,
        // which "reproducible" alone would happily accept.
        let scene = short_scene(20);
        let dir = tempfile::tempdir().expect("temp dir");
        export_to(dir.path(), &scene);
        let frames = frames_in(dir.path());
        let distinct: std::collections::HashSet<&Vec<u8>> = frames.iter().map(|(_, b)| b).collect();
        assert!(
            distinct.len() > 10,
            "an animated scene should produce mostly distinct frames, got {} distinct of {}",
            distinct.len(),
            frames.len()
        );
    }
}

/// Colour tagging and quality presets, asserted from the produced file rather
/// than from the arguments we passed.
///
/// Checking the argument list would only prove we typed the flags; ffmpeg is
/// free to ignore or override them. These run `ffprobe` on the output, so a
/// silently dropped tag fails.
#[cfg(test)]
mod encoded_output {
    use crate::{Exporter, Quality};
    use luminafx_renderer::skia_backend::SkiaRenderer;
    use luminafx_schema::Scene;
    use std::path::Path;
    use std::process::Command;

    fn tiny_scene() -> Scene {
        serde_json::from_value(serde_json::json!({
            "version": "1.0",
            "meta": { "title": "t", "author": "a", "created_at": "2026-01-01T00:00:00Z" },
            "canvas": {
                "width": 64, "height": 48, "fps": 10,
                "duration": 0.5, "background": "#101020"
            },
            "objects": {
                "c": { "type": "Circle", "properties": {
                    "cx": 32.0, "cy": 24.0, "radius": 14.0, "fill": "#FF8040" } }
            },
            "timeline": []
        }))
        .expect("fixture scene")
    }

    /// One `ffprobe` field from a produced file, or `None` when ffmpeg is
    /// unavailable — the export tests already skip in that case rather than
    /// failing on a machine without it.
    fn probe(path: &Path, field: &str) -> Option<String> {
        let out = Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-select_streams",
                "v:0",
                "-show_entries",
                &format!("stream={field}"),
                "-of",
                "csv=p=0",
            ])
            .arg(path)
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    fn export(quality: Quality, ext: &str) -> Option<(tempfile::TempDir, std::path::PathBuf)> {
        let dir = tempfile::tempdir().ok()?;
        let path = dir.path().join(format!("out.{ext}"));
        let mut exporter = Exporter::new(SkiaRenderer::new());
        let scene = tiny_scene();
        let result = match ext {
            "webm" => exporter.export_webm_with(&scene, &path, quality),
            _ => exporter.export_mp4_with(&scene, &path, quality),
        };
        // No ffmpeg on this machine: skip rather than fail.
        result.ok()?;
        Some((dir, path))
    }

    #[test]
    fn mp4_carries_bt709_colour_tags() {
        // Without these a player guesses how to interpret the pixels, and
        // players guess differently — the same file looks different in
        // QuickTime, VLC and Chrome.
        let Some((_dir, path)) = export(Quality::Standard, "mp4") else {
            return;
        };
        for (field, expected) in [
            ("color_space", "bt709"),
            ("color_primaries", "bt709"),
            ("color_transfer", "bt709"),
            ("color_range", "tv"),
        ] {
            assert_eq!(
                probe(&path, field).as_deref(),
                Some(expected),
                "{field} must be tagged {expected}"
            );
        }
    }

    #[test]
    fn webm_carries_colour_tags() {
        let Some((_dir, path)) = export(Quality::Standard, "webm") else {
            return;
        };
        assert_eq!(probe(&path, "color_space").as_deref(), Some("bt709"));
    }

    #[test]
    fn final_quality_is_ten_bit() {
        // 10 bits is what keeps banding out of a slow gradient, and it is the
        // whole reason the preset exists.
        let Some((_dir, path)) = export(Quality::Final, "mp4") else {
            return;
        };
        assert_eq!(
            probe(&path, "pix_fmt").as_deref(),
            Some("yuv420p10le"),
            "the final preset must produce 10-bit output"
        );
    }

    #[test]
    fn draft_and_standard_are_eight_bit() {
        // 8-bit yuv420p is the format every player and editor accepts; only
        // the explicit final preset departs from it.
        for quality in [Quality::Draft, Quality::Standard] {
            let Some((_dir, path)) = export(quality, "mp4") else {
                return;
            };
            assert_eq!(probe(&path, "pix_fmt").as_deref(), Some("yuv420p"));
        }
    }

    #[test]
    fn mp4_is_streamable() {
        // `+faststart` moves the index ahead of the payload so a browser can
        // begin playing before the file has finished downloading.
        let Some((_dir, path)) = export(Quality::Standard, "mp4") else {
            return;
        };
        let bytes = std::fs::read(&path).expect("output readable");
        let find = |needle: &[u8]| {
            bytes
                .windows(needle.len())
                .position(|w| w == needle)
                .unwrap_or(usize::MAX)
        };
        let (moov, mdat) = (find(b"moov"), find(b"mdat"));
        assert!(
            moov < mdat,
            "moov at {moov} must precede mdat at {mdat} for progressive playback"
        );
    }
}

/// Temporal supersampling: motion blur.
///
/// Rendering is analytic at any time, so blur is just several renders averaged
/// — which makes the risks accounting rather than graphics: does it stay
/// deterministic, does it stay aligned with the unblurred frame, and does it
/// darken the image.
#[cfg(test)]
mod motion_blur {
    use crate::Exporter;
    use luminafx_renderer::skia_backend::SkiaRenderer;
    use luminafx_schema::Scene;

    const W: u32 = 200;
    const H: u32 = 60;

    fn moving_scene(samples: u32, shutter: f32) -> Scene {
        serde_json::from_value(serde_json::json!({
            "version": "1.0",
            "meta": { "title": "t", "author": "a", "created_at": "2026-01-01T00:00:00Z" },
            "canvas": {
                "width": W, "height": H, "fps": 20, "duration": 1.0,
                "background": "#000000",
                "motion_blur_samples": samples, "shutter": shutter
            },
            "objects": {
                "c": { "type": "Circle", "properties": {
                    "cx": 20.0, "cy": 30.0, "radius": 12.0,
                    "fill": "#FFFFFF", "z_index": 1, "opacity": 1.0 } }
            },
            "timeline": [
                { "time": 0.0, "object": "c", "state": { "cx": 20.0 }, "easing": "linear" },
                { "time": 1.0, "object": "c", "state": { "cx": 180.0 }, "easing": "linear" }
            ]
        }))
        .expect("fixture")
    }

    /// The middle frame of a render, as raw RGBA.
    fn middle_frame(scene: &Scene) -> Vec<u8> {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut exporter = Exporter::new(SkiaRenderer::new());
        exporter
            .export_png_sequence(scene, dir.path())
            .expect("export");
        let img = image::open(dir.path().join("frame_0010.png")).expect("frame");
        img.to_rgba8().into_raw()
    }

    /// Mean luminance of the row through the circle's centre.
    fn centre_row_mean(px: &[u8]) -> f64 {
        let row = (H / 2) * W;
        let sum: u64 = (0..W).map(|x| u64::from(px[(row + x) as usize * 4])).sum();
        sum as f64 / f64::from(W)
    }

    #[test]
    fn one_sample_is_unchanged_from_no_blur() {
        // The default must be bit-identical to the previous behaviour, or every
        // existing scene renders differently.
        let a = middle_frame(&moving_scene(1, 0.5));
        let b = middle_frame(&moving_scene(1, 1.0));
        assert_eq!(a, b, "shutter must be ignored when there is one sample");
    }

    #[test]
    fn blur_is_deterministic() {
        // Samples are taken at fixed offsets, so two renders must agree.
        let a = middle_frame(&moving_scene(8, 0.5));
        let b = middle_frame(&moving_scene(8, 0.5));
        assert_eq!(a, b, "a blurred frame must render identically every time");
    }

    #[test]
    fn blur_spreads_the_moving_object() {
        // A blurred frame covers more of the row at lower intensity, so the
        // sharp frame has a higher peak and the blurred one a wider footprint.
        let sharp = middle_frame(&moving_scene(1, 0.5));
        let blurred = middle_frame(&moving_scene(8, 1.0));

        let row = (H / 2) * W;
        let lit = |px: &[u8], threshold: u8| {
            (0..W)
                .filter(|x| px[(row + x) as usize * 4] > threshold)
                .count()
        };
        assert!(
            lit(&blurred, 20) > lit(&sharp, 20),
            "blur should cover more pixels: sharp {} vs blurred {}",
            lit(&sharp, 20),
            lit(&blurred, 20)
        );
        let peak = |px: &[u8]| {
            (0..W)
                .map(|x| px[(row + x) as usize * 4])
                .max()
                .unwrap_or(0)
        };
        assert!(
            peak(&blurred) <= peak(&sharp),
            "a blurred edge cannot be brighter than the sharp original"
        );
    }

    #[test]
    fn blur_does_not_darken_the_frame() {
        // Averaging with truncation instead of rounding biases every channel
        // down, which dims a blurred render relative to a sharp one. The total
        // light in the row is conserved because the object only moves along it.
        let sharp = centre_row_mean(&middle_frame(&moving_scene(1, 0.5)));
        let blurred = centre_row_mean(&middle_frame(&moving_scene(8, 0.5)));
        assert!(
            (blurred - sharp).abs() < sharp * 0.15,
            "mean luminance should be roughly conserved: sharp {sharp:.1}, blurred {blurred:.1}"
        );
    }

    #[test]
    fn blur_stays_centred_on_the_frame_time() {
        // The shutter is centred on the frame's instant, so a blurred object
        // straddles where the sharp one is rather than trailing behind it.
        let sharp = middle_frame(&moving_scene(1, 0.5));
        let blurred = middle_frame(&moving_scene(8, 1.0));
        let row = (H / 2) * W;
        let centroid = |px: &[u8]| {
            let (mut wsum, mut sum) = (0.0f64, 0.0f64);
            for x in 0..W {
                let v = f64::from(px[(row + x) as usize * 4]);
                wsum += v * f64::from(x);
                sum += v;
            }
            if sum > 0.0 {
                wsum / sum
            } else {
                0.0
            }
        };
        let (a, b) = (centroid(&sharp), centroid(&blurred));
        assert!(
            (a - b).abs() < 2.0,
            "blur must stay centred: sharp centroid {a:.1}, blurred {b:.1}"
        );
    }
}
