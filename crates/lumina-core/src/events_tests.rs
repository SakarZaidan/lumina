#[cfg(test)]
mod tests {
    use crate::{Event, EventBus, Timeline};
    use lumina_schema::{Action, Canvas, EventEntry, Meta, Scene};
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
            },
            assets: Default::default(),
            objects: Default::default(),
            timeline: vec![],
            events,
            camera: None,
        }
    }

    fn fire(bus: &mut EventBus, timeline: &mut Timeline, object: &str, trigger: &str) {
        let _ = bus.process_event(
            &Event {
                object_id: object.into(),
                trigger: trigger.into(),
                payload: None,
            },
            timeline,
        );
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
        let scene = scene_with_events(vec![EventEntry {
            object: "vec".into(),
            trigger: "hover_enter".into(),
            action: Action::SetProperty {
                target: "vec".into(),
                property: "color".into(),
                value: json!("#F39C12"),
            },
        }]);
        let mut bus = EventBus::new(&scene);
        let mut timeline = Timeline::from_scene(&scene);
        fire(&mut bus, &mut timeline, "vec", "hover_enter");
        assert_eq!(
            timeline.overrides.get("vec").and_then(|m| m.get("color")),
            Some(&json!("#F39C12"))
        );
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
