#[cfg(test)]
mod tests {
    use crate::scene_patch::{apply_patch, ScenePatch};
    use lumina_schema::{
        Action, Canvas, CircleProps, EventEntry, GroupProps, Meta, Object, Scene, TimelineEntry,
    };
    use serde_json::json;

    fn base_scene() -> Scene {
        let mut objects = std::collections::HashMap::new();
        objects.insert(
            "c1".into(),
            Object::Circle(CircleProps {
                cx: 10.0,
                cy: 10.0,
                radius: 5.0,
                z_index: 0,
                fill: "#FFFFFF".into(),
                stroke: None,
                stroke_width: 0.0,
                shadow: None,
                opacity: 1.0,
            }),
        );
        Scene {
            version: "1.0".into(),
            meta: Meta {
                title: "Patch Test".into(),
                author: "test".into(),
                created_at: "now".into(),
            },
            canvas: Canvas {
                width: 100,
                height: 100,
                fps: 30,
                duration: 2.0,
                background: "#000000".into(),
            },
            assets: Default::default(),
            objects,
            timeline: vec![TimelineEntry {
                time: 1.0,
                object: "c1".into(),
                state: json!({ "opacity": 0.0 }),
                easing: "linear".into(),
                easing_params: None,
            }],
            events: vec![],
            camera: None,
        }
    }

    fn patch(json_str: serde_json::Value) -> ScenePatch {
        serde_json::from_value(json_str).expect("parse patch")
    }

    #[test]
    fn test_add_object() {
        let mut scene = base_scene();
        let p = patch(json!({
            "patches": [{
                "op": "add_object",
                "id": "c2",
                "type": "Circle",
                "properties": { "cx": 50.0, "cy": 50.0, "radius": 20.0, "fill": "#FF0000" }
            }]
        }));
        apply_patch(&mut scene, &p).unwrap();
        assert!(scene.objects.contains_key("c2"));
    }

    #[test]
    fn test_remove_object_cascades() {
        let mut scene = base_scene();
        // Add a group referencing c1 and an event targeting c1.
        scene.objects.insert(
            "g".into(),
            Object::Group(GroupProps {
                children: vec!["c1".into()],
                x: 0.0,
                y: 0.0,
                z_index: 0,
                scale: 1.0,
                rotation: 0.0,
                opacity: 1.0,
            }),
        );
        scene.events.push(EventEntry {
            object: "c1".into(),
            trigger: "click".into(),
            action: Action::Pause,
        });

        let p = patch(json!({ "patches": [{ "op": "remove_object", "id": "c1" }] }));
        apply_patch(&mut scene, &p).unwrap();

        assert!(!scene.objects.contains_key("c1"));
        assert!(scene.timeline.iter().all(|t| t.object != "c1"));
        assert!(scene.events.iter().all(|e| e.object != "c1"));
        if let Some(Object::Group(g)) = scene.objects.get("g") {
            assert!(!g.children.contains(&"c1".to_string()));
        } else {
            panic!("group missing");
        }
    }

    #[test]
    fn test_update_property() {
        let mut scene = base_scene();
        let p = patch(json!({
            "patches": [{ "op": "update_property", "object": "c1", "property": "radius", "value": 42.0 }]
        }));
        apply_patch(&mut scene, &p).unwrap();
        if let Some(Object::Circle(c)) = scene.objects.get("c1") {
            assert_eq!(c.radius, 42.0);
        } else {
            panic!("c1 missing");
        }
    }

    #[test]
    fn test_add_and_update_canvas_and_keyframe() {
        let mut scene = base_scene();
        let p = patch(json!({
            "patches": [
                { "op": "add_keyframe", "object": "c1", "keyframe": { "time": 0.5, "state": { "opacity": 0.5 }, "easing": "ease_in_quad" } },
                { "op": "update_canvas", "duration": 5.0, "background": "#101010" }
            ]
        }));
        apply_patch(&mut scene, &p).unwrap();
        assert_eq!(scene.canvas.duration, 5.0);
        assert_eq!(scene.canvas.background, "#101010");
        // Timeline stays time-sorted: the new t=0.5 keyframe is first.
        assert_eq!(scene.timeline[0].time, 0.5);
    }

    #[test]
    fn test_update_property_unknown_object_errors() {
        let mut scene = base_scene();
        let p = patch(json!({
            "patches": [{ "op": "update_property", "object": "nope", "property": "radius", "value": 1.0 }]
        }));
        assert!(apply_patch(&mut scene, &p).is_err());
    }
}
