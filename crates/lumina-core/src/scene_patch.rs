//! Semantic scene patching (blueprint §15).
//!
//! Unlike the server's RFC-6902 JSON Patch endpoint (which operates on raw
//! JSON), `ScenePatch` expresses edits in the domain language of the engine —
//! add an object, add a keyframe, update a canvas field — and applies them to a
//! strongly-typed [`Scene`]. This is the format an AI agent uses to extend a
//! scene it previously generated without resending the whole document.

use lumina_schema::{Canvas, EventEntry, Object, Scene, TimelineEntry};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// Why a patch operation could not be applied.
#[derive(Debug, Error)]
pub enum PatchError {
    /// The referenced object id does not exist in the scene.
    #[error("object '{0}' not found")]
    ObjectNotFound(String),
    /// The provided type/properties do not form a valid object.
    #[error("invalid object for '{0}': {1}")]
    InvalidObject(String, String),
    /// No keyframe exists for the object at the given time.
    #[error("keyframe at t={1} not found for object '{0}'")]
    KeyframeNotFound(String, f32),
    /// (De)serialization of an object or property failed.
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

/// A keyframe body (the per-op `object` provides the target id).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyframeSpec {
    /// Time in seconds.
    pub time: f32,
    /// Property values to key at this time.
    pub state: Value,
    /// Easing toward this keyframe (default `linear`).
    #[serde(default = "default_easing")]
    pub easing: String,
    /// Parameters for `cubic_bezier`/`spline` easings.
    #[serde(default)]
    pub easing_params: Option<Value>,
}

fn default_easing() -> String {
    "linear".to_string()
}

/// One semantic edit, tagged by `"op"` in JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op")]
pub enum PatchOp {
    /// Add a new object to the scene.
    #[serde(rename = "add_object")]
    AddObject {
        /// Id for the new object (must be unique).
        id: String,
        /// LSF object type name (e.g. `"Circle"`).
        #[serde(rename = "type")]
        object_type: String,
        /// The object's `properties` block.
        properties: Value,
    },
    /// Remove an object plus its timeline entries, events, and group
    /// memberships.
    #[serde(rename = "remove_object")]
    RemoveObject {
        /// Id of the object to remove.
        id: String,
    },
    /// Set one property on an object's initial state.
    #[serde(rename = "update_property")]
    UpdateProperty {
        /// Target object id.
        object: String,
        /// Property name.
        property: String,
        /// New value.
        value: Value,
    },
    /// Append a keyframe to an object's timeline.
    #[serde(rename = "add_keyframe")]
    AddKeyframe {
        /// Target object id.
        object: String,
        /// The keyframe to add.
        keyframe: KeyframeSpec,
    },
    /// Remove the keyframe at an exact time.
    #[serde(rename = "remove_keyframe")]
    RemoveKeyframe {
        /// Target object id.
        object: String,
        /// Keyframe time in seconds (exact match).
        time: f32,
    },
    /// Replace the state of the keyframe at an exact time.
    #[serde(rename = "update_keyframe")]
    UpdateKeyframe {
        /// Target object id.
        object: String,
        /// Keyframe time in seconds (exact match).
        time: f32,
        /// Replacement state.
        state: Value,
    },
    /// Add an interactive event entry.
    #[serde(rename = "add_event")]
    AddEvent {
        /// The event to append.
        event: EventEntry,
    },
    /// Remove the event matching object + trigger.
    #[serde(rename = "remove_event")]
    RemoveEvent {
        /// Target object id.
        object: String,
        /// Trigger name (e.g. `"click"`).
        trigger: String,
    },
    /// Update any subset of canvas fields.
    #[serde(rename = "update_canvas")]
    UpdateCanvas {
        /// New canvas width, if changing.
        #[serde(default)]
        width: Option<u32>,
        /// New canvas height, if changing.
        #[serde(default)]
        height: Option<u32>,
        /// New frame rate, if changing.
        #[serde(default)]
        fps: Option<u32>,
        /// New duration in seconds, if changing.
        #[serde(default)]
        duration: Option<f32>,
        /// New background color, if changing.
        #[serde(default)]
        background: Option<String>,
    },
}

/// An ordered list of semantic edits against a scene.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenePatch {
    /// Optional id of the scene this patch was authored against.
    #[serde(default)]
    pub base_scene_id: Option<String>,
    /// Operations, applied in order.
    pub patches: Vec<PatchOp>,
}

/// Apply a semantic patch to a scene in place. Operations are applied in order;
/// the first failure aborts (the scene may be partially modified, so callers
/// that need atomicity should clone first).
pub fn apply_patch(scene: &mut Scene, patch: &ScenePatch) -> Result<(), PatchError> {
    for op in &patch.patches {
        apply_op(scene, op)?;
    }
    Ok(())
}

fn apply_op(scene: &mut Scene, op: &PatchOp) -> Result<(), PatchError> {
    match op {
        PatchOp::AddObject {
            id,
            object_type,
            properties,
        } => {
            let tagged = serde_json::json!({ "type": object_type, "properties": properties });
            let object: Object = serde_json::from_value(tagged)
                .map_err(|e| PatchError::InvalidObject(id.clone(), e.to_string()))?;
            scene.objects.insert(id.clone(), object);
        }
        PatchOp::RemoveObject { id } => {
            scene.objects.remove(id);
            // Cascade: drop timeline + event entries that target the object, and
            // remove it from any group's child list.
            scene.timeline.retain(|t| &t.object != id);
            scene.events.retain(|e| &e.object != id);
            for obj in scene.objects.values_mut() {
                if let Object::Group(g) = obj {
                    g.children.retain(|c| c != id);
                }
            }
        }
        PatchOp::UpdateProperty {
            object,
            property,
            value,
        } => {
            let obj = scene
                .objects
                .get_mut(object)
                .ok_or_else(|| PatchError::ObjectNotFound(object.clone()))?;
            // Round-trip through JSON so any typed property can be set generically.
            let mut v = serde_json::to_value(&*obj)?;
            if let Some(props) = v.get_mut("properties").and_then(|p| p.as_object_mut()) {
                props.insert(property.clone(), value.clone());
            }
            *obj = serde_json::from_value(v)
                .map_err(|e| PatchError::InvalidObject(object.clone(), e.to_string()))?;
        }
        PatchOp::AddKeyframe { object, keyframe } => {
            scene.timeline.push(TimelineEntry {
                time: keyframe.time,
                object: object.clone(),
                state: keyframe.state.clone(),
                easing: keyframe.easing.clone(),
                easing_params: keyframe.easing_params.clone(),
            });
            scene.timeline.sort_by(|a, b| {
                a.time
                    .partial_cmp(&b.time)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        PatchOp::RemoveKeyframe { object, time } => {
            let before = scene.timeline.len();
            scene
                .timeline
                .retain(|t| !(&t.object == object && (t.time - time).abs() < 1e-6));
            if scene.timeline.len() == before {
                return Err(PatchError::KeyframeNotFound(object.clone(), *time));
            }
        }
        PatchOp::UpdateKeyframe {
            object,
            time,
            state,
        } => {
            let entry = scene
                .timeline
                .iter_mut()
                .find(|t| &t.object == object && (t.time - time).abs() < 1e-6)
                .ok_or_else(|| PatchError::KeyframeNotFound(object.clone(), *time))?;
            entry.state = state.clone();
        }
        PatchOp::AddEvent { event } => {
            scene.events.push(event.clone());
        }
        PatchOp::RemoveEvent { object, trigger } => {
            scene
                .events
                .retain(|e| !(&e.object == object && &e.trigger == trigger));
        }
        PatchOp::UpdateCanvas {
            width,
            height,
            fps,
            duration,
            background,
        } => {
            let c: &mut Canvas = &mut scene.canvas;
            if let Some(w) = width {
                c.width = *w;
            }
            if let Some(h) = height {
                c.height = *h;
            }
            if let Some(f) = fps {
                c.fps = *f;
            }
            if let Some(d) = duration {
                c.duration = *d;
            }
            if let Some(bg) = background {
                c.background = bg.clone();
            }
        }
    }
    Ok(())
}
