#[cfg(test)]
mod tests {
    use crate::{Event, EventBus, Timeline};
    use luminafx_schema::{Action, Canvas, EventEntry, Meta, Scene};
    use serde_json::json;

    fn scene_with_events(events: Vec<EventEntry>) -> Scene {
        Scene {
            version: "1.0".into(),
            meta: Meta {
                title: "Events Test".into(),
                author: "test".into(),
                created_at: "now".into(),
            },
            canvas: Canvas {
                width: 64,
                height: 64,
                fps: 30,
                duration: 5.0,
                background: "#000000".into(),
                motion_blur_samples: 1,
                shutter: 0.5,
            },
            assets: Default::default(),
            objects: Default::default(),
            timeline: vec![],
            events,
            camera: None,
        }
    }

    fn fire(
        bus: &mut EventBus,
        timeline: &mut Timeline,
        object: &str,
        trigger: &str,
    ) -> crate::events::EventOutcome {
        bus.process_event(
            &Event {
                object_id: object.into(),
                trigger: trigger.into(),
                payload: None,
            },
            timeline,
        )
    }

    #[test]
    fn test_jump_to_time_seeks_playhead() {
        let scene = scene_with_events(vec![EventEntry {
            object: "btn".into(),
            trigger: "click".into(),
            action: Action::JumpToTime { value: 3.5 },
        }]);
        let mut bus = EventBus::new(&scene);
        let mut timeline = Timeline::from_scene(&scene);
        assert_eq!(bus.playback.current_time, 0.0);
        fire(&mut bus, &mut timeline, "btn", "click");
        assert_eq!(bus.playback.current_time, 3.5);
    }

    #[test]
    fn test_play_from_and_pause() {
        let scene = scene_with_events(vec![
            EventEntry {
                object: "obj".into(),
                trigger: "double_click".into(),
                action: Action::PlayFrom { value: 2.0 },
            },
            EventEntry {
                object: "obj".into(),
                trigger: "click".into(),
                action: Action::Pause,
            },
        ]);
        let mut bus = EventBus::new(&scene);
        let mut timeline = Timeline::from_scene(&scene);

        fire(&mut bus, &mut timeline, "obj", "double_click");
        assert!(bus.playback.playing);
        assert_eq!(bus.playback.current_time, 2.0);

        fire(&mut bus, &mut timeline, "obj", "click");
        assert!(!bus.playback.playing);
    }

    #[test]
    fn test_set_property_creates_override() {
        let mut scene = scene_with_events(vec![EventEntry {
            object: "vec".into(),
            trigger: "hover_enter".into(),
            action: Action::SetProperty {
                target: "vec".into(),
                property: "color".into(),
                value: json!("#F39C12"),
            },
        }]);
        // The action needs a real object with that property: an override for
        // one the scene does not have is refused rather than stored.
        scene.objects.insert(
            "vec".into(),
            serde_json::from_value(json!({
                "type": "Arrow", "properties": { "from": [0, 0], "to": [10, 10] }
            }))
            .expect("arrow"),
        );
        let mut bus = EventBus::new(&scene);
        let mut timeline = Timeline::from_scene(&scene);
        let outcome = fire(&mut bus, &mut timeline, "vec", "hover_enter");
        assert!(outcome.rejected.is_empty(), "{:?}", outcome.rejected);
        assert_eq!(
            timeline.overrides.get("vec").and_then(|m| m.get("color")),
            Some(&json!("#F39C12"))
        );
    }

    #[test]
    fn an_action_the_engine_cannot_apply_comes_back_with_the_reason() {
        // It used to be stored and then ignored by every reader, which looks
        // exactly like an event that never fired.
        let mut scene = scene_with_events(vec![EventEntry {
            object: "dot".into(),
            trigger: "click".into(),
            action: Action::SetProperty {
                target: "dot".into(),
                property: "colour".into(),
                value: json!("#F39C12"),
            },
        }]);
        scene.objects.insert(
            "dot".into(),
            serde_json::from_value(json!({
                "type": "Circle", "properties": { "cx": 1, "cy": 1, "radius": 5 }
            }))
            .expect("circle"),
        );
        let mut bus = EventBus::new(&scene);
        let mut timeline = Timeline::from_scene(&scene);
        let outcome = fire(&mut bus, &mut timeline, "dot", "click");
        assert_eq!(outcome.rejected.len(), 1, "{:?}", outcome.rejected);
        assert_eq!(outcome.rejected[0].error.code, "UNKNOWN_PROPERTY");
        assert!(timeline.overrides.is_empty(), "it was stored anyway");
    }

    #[test]
    fn test_emit_custom_returns_payload_with_drag_substitution() {
        let scene = scene_with_events(vec![EventEntry {
            object: "vec_a".into(),
            trigger: "drag".into(),
            action: Action::EmitCustom {
                event_name: "vector_moved".into(),
                payload: json!({ "from": "$drag.from", "to": "$drag.to" }),
            },
        }]);
        let mut bus = EventBus::new(&scene);
        let mut timeline = Timeline::from_scene(&scene);

        let outcome = bus.process_event(
            &Event {
                object_id: "vec_a".into(),
                trigger: "drag".into(),
                payload: Some(json!({ "from": [400.0, 540.0], "to": [700.0, 300.0] })),
            },
            &mut timeline,
        );

        assert_eq!(outcome.emitted.len(), 1);
        let e = &outcome.emitted[0];
        assert_eq!(e.event_name, "vector_moved");
        assert_eq!(e.payload["from"], json!([400.0, 540.0]));
        assert_eq!(e.payload["to"], json!([700.0, 300.0]));
    }

    #[test]
    fn test_unmatched_event_is_noop() {
        let scene = scene_with_events(vec![EventEntry {
            object: "a".into(),
            trigger: "click".into(),
            action: Action::JumpToTime { value: 1.0 },
        }]);
        let mut bus = EventBus::new(&scene);
        let mut timeline = Timeline::from_scene(&scene);
        let outcome = bus.process_event(
            &Event {
                object_id: "b".into(),
                trigger: "click".into(),
                payload: None,
            },
            &mut timeline,
        );
        assert!(outcome.actions.is_empty());
        assert_eq!(bus.playback.current_time, 0.0);
    }
}
