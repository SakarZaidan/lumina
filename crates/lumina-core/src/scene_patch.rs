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

#[derive(Debug, Error)]
pub enum PatchError {
    #[error("object '{0}' not found")]
    ObjectNotFound(String),
    #[error("invalid object for '{0}': {1}")]
    InvalidObject(String, String),
    #[error("keyframe at t={1} not found for object '{0}'")]
    KeyframeNotFound(String, f32),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

/// A keyframe body (the per-op `object` provides the target id).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyframeSpec {
    pub time: f32,
    pub state: Value,
    #[serde(default = "default_easing")]
    pub easing: String,
    #[serde(default)]
    pub easing_params: Option<Value>,
}

fn default_easing() -> String {
    "linear".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op")]
pub enum PatchOp {
    #[serde(rename = "add_object")]
    AddObject {
        id: String,
        #[serde(rename = "type")]
        object_type: String,
        properties: Value,
    },
    #[serde(rename = "remove_object")]
    RemoveObject { id: String },
    #[serde(rename = "update_property")]
    UpdateProperty {
        object: String,
        property: String,
        value: Value,
    },
    #[serde(rename = "add_keyframe")]
    AddKeyframe {
        object: String,
        keyframe: KeyframeSpec,
    },
    #[serde(rename = "remove_keyframe")]
    RemoveKeyframe { object: String, time: f32 },
    #[serde(rename = "update_keyframe")]
    UpdateKeyframe {
        object: String,
        time: f32,
        state: Value,
    },
    #[serde(rename = "add_event")]
    AddEvent { event: EventEntry },
    #[serde(rename = "remove_event")]
    RemoveEvent { object: String, trigger: String },
    #[serde(rename = "update_canvas")]
    UpdateCanvas {
        #[serde(default)]
        width: Option<u32>,
        #[serde(default)]
        height: Option<u32>,
        #[serde(default)]
        fps: Option<u32>,
        #[serde(default)]
        duration: Option<f32>,
        #[serde(default)]
        background: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenePatch {
    #[serde(default)]
    pub base_scene_id: Option<String>,
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
