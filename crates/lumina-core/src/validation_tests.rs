#[cfg(test)]
mod tests {
    use crate::validation::validate_scene_data;
    use lumina_schema::{Camera, CameraState, CameraTimelineEntry, Scene, TimelineEntry};

    fn scene_with_easing(easing: &str) -> Scene {
        let json = serde_json::json!({
            "version": "1.0",
            "meta": { "title": "t", "author": "a", "created_at": "2026-01-01T00:00:00Z" },
            "canvas": { "width": 100, "height": 100, "fps": 30, "duration": 2.0, "background": "#000000" },
            "objects": {
                "dot": { "type": "Circle", "properties": { "cx": 10, "cy": 10, "radius": 5 } }
            },
            "timeline": []
        });
        let mut scene: Scene = serde_json::from_value(json).unwrap();
        scene.timeline.push(TimelineEntry {
            time: 1.0,
            object: "dot".to_string(),
            state: serde_json::json!({ "cx": 50 }),
            easing: easing.to_string(),
            easing_params: None,
        });
        scene
    }

    #[test]
    fn unknown_easing_is_rejected_with_suggestion() {
        let result = validate_scene_data(&scene_with_easing("ease_in_ou"));
        assert!(!result.valid);
        let err = result
            .errors
            .iter()
            .find(|e| e.code == "UNKNOWN_EASING")
            .expect("UNKNOWN_EASING error expected");
        assert_eq!(err.path, "$.timeline[0].easing");
        assert!(
            err.fix_suggestion.contains("ease_in_out"),
            "suggestion should name the nearest easing, got: {}",
            err.fix_suggestion
        );
    }

    #[test]
    fn known_easing_passes() {
        let result = validate_scene_data(&scene_with_easing("ease_out_bounce"));
        assert!(result.valid, "errors: {:?}", result.errors);
    }

    #[test]
    fn parameterized_easing_without_params_warns() {
        let result = validate_scene_data(&scene_with_easing("cubic_bezier"));
        assert!(result.valid, "missing params is a warning, not an error");
        assert!(result
            .warnings
            .iter()
            .any(|w| w.code == "MISSING_EASING_PARAMS"));
    }

    #[test]
    fn camera_easing_is_validated() {
        let mut scene = scene_with_easing("linear");
        scene.camera = Some(Camera {
            timeline: vec![CameraTimelineEntry {
                time: 0.0,
                state: CameraState {
                    x: 0.0,
                    y: 0.0,
                    zoom: 1.0,
                },
                easing: "zoom_zoom".to_string(),
            }],
        });
        let result = validate_scene_data(&scene);
        assert!(!result.valid);
        assert!(result
            .errors
            .iter()
            .any(|e| e.code == "UNKNOWN_EASING" && e.path == "$.camera.timeline[0].easing"));
    }
}
