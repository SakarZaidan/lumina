use lumina_schema::{EventEntry, Action as SchemaAction, Scene};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub object_id: String,
    pub trigger: String,
    pub payload: Option<Value>,
}

pub struct EventBus {
    event_definitions: Vec<EventEntry>,
}

impl EventBus {
    pub fn new(scene: &Scene) -> Self {
        Self {
            event_definitions: scene.events.clone(),
        }
    }

    pub fn process_event(&self, event: &Event, timeline: &mut crate::timeline::Timeline) -> Vec<SchemaAction> {
        let mut triggered_actions = Vec::new();

        for entry in &self.event_definitions {
            if entry.object == event.object_id && entry.trigger == event.trigger {
                triggered_actions.push(entry.action.clone());
                self.execute_action(&entry.action, timeline);
            }
        }

        triggered_actions
    }

    fn execute_action(&self, action: &SchemaAction, timeline: &mut crate::timeline::Timeline) {
        match action {
            SchemaAction::JumpToTime { value: _ } => {
                // Runtime handle
            }
            SchemaAction::SetProperty { target, property, value } => {
                timeline.override_property(target, property, value.clone());
            }
        }
    }
}
