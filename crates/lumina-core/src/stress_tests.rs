#[cfg(test)]
mod tests {
    use crate::scene::SceneGraph;
    use lumina_schema::{Canvas, CircleProps, Meta, Object, Scene};
    use std::collections::HashMap;

    #[test]
    fn test_rendering_volume_2000_objects() {
        let mut objects = HashMap::new();
        for i in 0..2000 {
            objects.insert(
                format!("circle_{}", i),
                lumina_schema::Object::Circle(CircleProps {
                    cx: 0.0,
                    cy: 0.0,
                    radius: 5.0,
                    z_index: 0,
                    fill: "#FFFFFF".into(),
                    stroke: None,
                    stroke_width: 1.0,
                    shadow: None,
                    opacity: 1.0,
                }),
            );
        }
        let scene = Scene {
            version: "1.0".into(),
            meta: Meta {
                title: "Stress".into(),
                author: "Test".into(),
                created_at: "now".into(),
            },
            canvas: Canvas {
                width: 1920,
                height: 1080,
                fps: 60,
                duration: 1.0,
                background: "#000000".into(),
            },
            assets: Default::default(),
            objects,
            timeline: vec![],
            events: vec![],
            camera: None,
        };

        let start = std::time::Instant::now();
        let _graph = SceneGraph::from_scene(&scene);
        let duration = start.elapsed();

        assert!(
            duration.as_millis() < 16,
            "Scene graph construction too slow: {}ms",
            duration.as_millis()
        );
    }

    #[test]
    fn test_memory_churn_stress_test() {
        let mut objects = HashMap::new();
        for i in 0..2000 {
            objects.insert(
                format!("obj_{}", i),
                lumina_schema::Object::Circle(CircleProps {
                    cx: 0.0,
                    cy: 0.0,
                    radius: 1.0,
                    z_index: 0,
                    fill: "#FFF".into(),
                    stroke: None,
                    stroke_width: 1.0,
                    shadow: None,
                    opacity: 1.0,
                }),
            );
        }

        let mut scene_graph = SceneGraph {
            objects,
            root_objects: vec![],
        };

        for frame in 0..100 {
            for i in 0..500 {
                let id_to_remove = format!("obj_{}", (frame * 500 + i) % 2000);
                scene_graph.remove_object(&id_to_remove);
            }
            for i in 0..500 {
                let id_to_add = format!("obj_{}", (frame * 500 + i) % 2000);
                scene_graph.add_object(
                    id_to_add,
                    Object::Circle(CircleProps {
                        cx: 0.0,
                        cy: 0.0,
                        radius: 1.0,
                        z_index: 0,
                        fill: "#FFF".into(),
                        stroke: None,
                        stroke_width: 1.0,
                        shadow: None,
                        opacity: 1.0,
                    }),
                );
            }
        }
        assert_eq!(scene_graph.objects.len(), 2000);
    }

    #[test]
    fn test_interactivity_event_throughput() {
        use crate::events::Event;
        use crate::timeline::Timeline;

        let mut objects = HashMap::new();
        objects.insert(
            "eyes".into(),
            Object::Circle(CircleProps {
                cx: 0.0,
                cy: 0.0,
                radius: 10.0,
                z_index: 0,
                fill: "#FFF".into(),
                stroke: None,
                stroke_width: 1.0,
                shadow: None,
                opacity: 1.0,
            }),
        );

        let scene = Scene {
            version: "1.0".into(),
            meta: Meta {
                title: "Mascot".into(),
                author: "Dev".into(),
                created_at: "now".into(),
            },
            canvas: Canvas {
                width: 100,
                height: 100,
                fps: 60,
                duration: 1.0,
                background: "#000".into(),
            },
            assets: Default::default(),
            objects,
            timeline: vec![],
            events: vec![lumina_schema::EventEntry {
                object: "eyes".into(),
                trigger: "mouse_move".into(),
                action: lumina_schema::Action::SetProperty {
                    target: "eyes".into(),
                    property: "rotation".into(),
                    value: serde_json::json!(45.0),
                },
            }],
            camera: None,
        };

        let mut timeline = Timeline::from_scene(&scene);
        let mut event_bus = crate::events::EventBus::new(&scene);

        for _ in 0..1000 {
            let event = Event {
                object_id: "eyes".into(),
                trigger: "mouse_move".into(),
                payload: None,
            };
            event_bus.process_event(&event, &mut timeline);
        }

        let state = timeline.get_state_at(0.0);
        assert_eq!(state["eyes"]["rotation"], serde_json::json!(45.0));
    }
}
