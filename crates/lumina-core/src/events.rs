use lumina_schema::{Action as SchemaAction, EventEntry, Scene};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
/// An interaction delivered by the host (click, drag, hover, …).
pub struct Event {
    /// Id of the object the interaction targets.
    pub object_id: String,
    /// Trigger name as declared in the scene's `events` block (e.g. `"click"`).
    pub trigger: String,
    /// Host-supplied payload; `$drag.*` placeholders are substituted from it.
    pub payload: Option<Value>,
}

/// Mutable playback state owned by the event bus. The host render loop reads
/// `current_time`/`playing` after dispatching an event instead of driving the
/// clock itself, so actions like `jump_to_time`, `play_from` and `pause` take
/// effect.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackState {
    /// Current playhead position in seconds.
    pub current_time: f32,
    /// Whether the timeline is advancing.
    pub playing: bool,
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self {
            current_time: 0.0,
            playing: true,
        }
    }
}

/// The outcome of dispatching an event: the resolved actions (with `$drag`
/// placeholders substituted) plus the post-dispatch playback state and any
/// custom events emitted for the host.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventOutcome {
    /// Actions the host must apply (tooltips, property sets, …).
    pub actions: Vec<SchemaAction>,
    /// Playhead position after the event was handled.
    pub current_time: f32,
    /// Playback state after the event was handled.
    pub playing: bool,
    /// Custom events emitted toward the host application.
    pub emitted: Vec<EmittedEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// A custom event surfaced to the host via an `emit_custom` action.
pub struct EmittedEvent {
    /// Name declared in the scene's `emit_custom` action.
    pub event_name: String,
    /// Payload after `$drag.*` placeholder substitution.
    pub payload: Value,
}

/// Dispatches host interactions against the scene's `events` table and
/// owns the resulting playback state.
pub struct EventBus {
    event_definitions: Vec<EventEntry>,
    /// Current playback state (mutated by playback-control actions).
    pub playback: PlaybackState,
}

impl EventBus {
    /// Build a bus over the scene's declared events, starting paused at t=0.
    pub fn new(scene: &Scene) -> Self {
        Self {
            event_definitions: scene.events.clone(),
            playback: PlaybackState::default(),
        }
    }

    /// Dispatch an incoming interaction event. Every matching event definition
    /// (same object + trigger) fires its action. Returns an [`EventOutcome`] the
    /// host uses to update playback and react to emitted custom events.
    pub fn process_event(
        &mut self,
        event: &Event,
        timeline: &mut crate::timeline::Timeline,
    ) -> EventOutcome {
        let mut actions = Vec::new();
        let mut emitted = Vec::new();

        // Snapshot matched actions first so the borrow on `self.event_definitions`
        // doesn't overlap the `&mut self` execution.
        let matched: Vec<SchemaAction> = self
            .event_definitions
            .iter()
            .filter(|e| e.object == event.object_id && e.trigger == event.trigger)
            .map(|e| e.action.clone())
            .collect();

        for action in matched {
            let resolved = substitute_drag(&action, event.payload.as_ref());
            self.execute_action(&resolved, timeline, &mut emitted);
            actions.push(resolved);
        }

        EventOutcome {
            actions,
            current_time: self.playback.current_time,
            playing: self.playback.playing,
            emitted,
        }
    }

    fn execute_action(
        &mut self,
        action: &SchemaAction,
        timeline: &mut crate::timeline::Timeline,
        emitted: &mut Vec<EmittedEvent>,
    ) {
        match action {
            SchemaAction::JumpToTime { value } => {
                self.playback.current_time = *value;
            }
            SchemaAction::PlayFrom { value } => {
                self.playback.current_time = *value;
                self.playback.playing = true;
            }
            SchemaAction::Pause => {
                self.playback.playing = false;
            }
            SchemaAction::SetProperty {
                target,
                property,
                value,
            } => {
                timeline.override_property(target, property, value.clone());
            }
            // The current value is applied immediately via the override channel.
            // Hosts that want a smooth on-demand tween can read the returned
            // action and animate it themselves.
            SchemaAction::TweenTo {
                target,
                property,
                value,
                ..
            } => {
                timeline.override_property(target, property, value.clone());
            }
            SchemaAction::ShowTooltip { .. } => {
                // Purely a host-side overlay; no engine state changes.
            }
            SchemaAction::EmitCustom {
                event_name,
                payload,
            } => {
                emitted.push(EmittedEvent {
                    event_name: event_name.clone(),
                    payload: payload.clone(),
                });
            }
        }
    }
}

/// Replace `$drag.<key>` string placeholders inside an action with the matching
/// value from the event payload (e.g. `{"from": [...], "to": [...]}`). Returns a
/// cloned action with placeholders resolved; non-placeholder values pass through.
fn substitute_drag(action: &SchemaAction, payload: Option<&Value>) -> SchemaAction {
    let payload = match payload {
        Some(p) => p,
        None => return action.clone(),
    };
    let mut a = action.clone();
    match &mut a {
        SchemaAction::SetProperty { value, .. } | SchemaAction::TweenTo { value, .. } => {
            substitute_value(value, payload);
        }
        SchemaAction::EmitCustom { payload: p, .. } => {
            substitute_value(p, payload);
        }
        _ => {}
    }
    a
}

/// Recursively replace any string of the form `$drag.<key>` with
/// `payload[<key>]`.
fn substitute_value(value: &mut Value, payload: &Value) {
    match value {
        Value::String(s) => {
            if let Some(key) = s.strip_prefix("$drag.") {
                if let Some(replacement) = payload.get(key) {
                    *value = replacement.clone();
                }
            }
        }
        Value::Array(arr) => {
            for v in arr.iter_mut() {
                substitute_value(v, payload);
            }
        }
        Value::Object(map) => {
            for (_, v) in map.iter_mut() {
                substitute_value(v, payload);
            }
        }
        _ => {}
    }
}
