#[cfg(test)]
mod tests {
    use crate::Exporter;
    use lumina_renderer::skia_backend::SkiaRenderer;
    use lumina_schema::{Canvas, CircleProps, Meta, Object, Scene, TimelineEntry};
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
