use crate::interpolator::interpolate_value;
use lumina_schema::Scene;
use serde_json::Value;
use std::collections::HashMap;

pub struct Timeline {
    pub duration: f32,
    pub fps: u32,
    // object_id -> property_name -> keyframes (sorted by time)
    pub tracks: HashMap<String, HashMap<String, Vec<Keyframe>>>,
    // object_id -> property_name -> value (for interactive overrides)
    pub overrides: HashMap<String, HashMap<String, Value>>,
}

#[derive(Clone, Debug)]
pub struct Keyframe {
    pub time: f32,
    pub value: Value,
    pub easing: String,
}

impl Timeline {
    pub fn from_scene(scene: &Scene) -> Self {
        let mut tracks: HashMap<String, HashMap<String, Vec<Keyframe>>> = HashMap::new();

        // 1. Initialize with initial property values from objects
        for (id, obj) in &scene.objects {
            let initial_state = serde_json::to_value(obj).unwrap();
            if let Value::Object(props) = &initial_state["properties"] {
                for (prop_name, prop_value) in props {
                    let track = tracks.entry(id.clone()).or_default().entry(prop_name.clone()).or_default();
                    track.push(Keyframe {
                        time: 0.0,
                        value: prop_value.clone(),
                        easing: "linear".to_string(),
                    });
                }
            }
        }

        // 2. Add keyframes from timeline
        for entry in &scene.timeline {
            if let Value::Object(state) = &entry.state {
                for (prop_name, prop_value) in state {
                    let track = tracks.entry(entry.object.clone()).or_default().entry(prop_name.clone()).or_default();
                    track.push(Keyframe {
                        time: entry.time,
                        value: prop_value.clone(),
                        easing: entry.easing.clone(),
                    });
                }
            }
        }

        // 3. Sort keyframes by time
        for object_tracks in tracks.values_mut() {
            for track in object_tracks.values_mut() {
                track.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
            }
        }

        Self {
            duration: scene.canvas.duration,
            fps: scene.canvas.fps,
            tracks,
            overrides: HashMap::new(),
        }
    }

    pub fn override_property(&mut self, object_id: &str, property: &str, value: Value) {
        self.overrides
            .entry(object_id.to_string())
            .or_default()
            .insert(property.to_string(), value);
    }

    pub fn get_state_at(&self, time: f32) -> HashMap<String, Value> {
        let mut state: HashMap<String, HashMap<String, Value>> = HashMap::new();

        // 1. Evaluate tracks
        for (obj_id, object_tracks) in &self.tracks {
            let obj_state = state.entry(obj_id.clone()).or_default();
            for (prop_name, track) in object_tracks {
                obj_state.insert(prop_name.clone(), self.evaluate_track(track, time));
            }
        }

        // 2. Apply overrides
        for (obj_id, obj_overrides) in &self.overrides {
            let obj_state = state.entry(obj_id.clone()).or_default();
            for (prop_name, value) in obj_overrides {
                obj_state.insert(prop_name.clone(), value.clone());
            }
        }

        state.into_iter()
            .map(|(k, v)| (k, Value::Object(v.into_iter().collect())))
            .collect()
    }

    fn evaluate_track(&self, track: &[Keyframe], time: f32) -> Value {
        if track.is_empty() {
            return Value::Null;
        }

        // Find the two keyframes surrounding the current time
        let mut lower = &track[0];
        let mut upper = &track[0];

        if time <= lower.time {
            return lower.value.clone();
        }

        if time >= track[track.len() - 1].time {
            return track[track.len() - 1].value.clone();
        }

        for i in 0..track.len() - 1 {
            if time >= track[i].time && time < track[i + 1].time {
                lower = &track[i];
                upper = &track[i + 1];
                break;
            }
        }

        if lower.time == upper.time {
            return lower.value.clone();
        }

        let t = (time - lower.time) / (upper.time - lower.time);
        interpolate_value(&lower.value, &upper.value, t, &upper.easing)
    }
}
