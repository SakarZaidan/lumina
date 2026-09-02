use crate::interpolator::interpolate_value;
use lumina_schema::Scene;
use serde_json::Value;
use std::collections::HashMap;

/// Per-object, per-property keyframe tracks; evaluation is deterministic
/// and scrub-safe (any time can be queried in any order).
pub struct Timeline {
    /// Scene duration in seconds (from the canvas block).
    pub duration: f32,
    /// Frames per second (from the canvas block).
    pub fps: u32,
    /// `object_id` → `property_name` → keyframes (sorted by time)
    pub tracks: HashMap<String, HashMap<String, Vec<Keyframe>>>,
    /// `object_id` → `property_name` → value (interactive overrides take precedence)
    pub overrides: HashMap<String, HashMap<String, Value>>,
}

#[derive(Clone, Debug)]
/// One keyframe on a property track.
pub struct Keyframe {
    /// Time in seconds.
    pub time: f32,
    /// Property value at this keyframe.
    pub value: Value,
    /// Easing applied when interpolating *toward* this keyframe.
    pub easing: String,
    /// Parameters for parameterized easings such as `cubic_bezier`.
    pub easing_params: Option<Value>,
}

impl Timeline {
    /// Build tracks from the scene: initial object properties become t=0
    /// keyframes, then timeline entries add theirs.
    pub fn from_scene(scene: &Scene) -> Self {
        let mut tracks: HashMap<String, HashMap<String, Vec<Keyframe>>> = HashMap::new();

        // Initialize with initial property values from objects
        for (id, obj) in &scene.objects {
            let initial_state = match serde_json::to_value(obj) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if let Value::Object(props) = &initial_state["properties"] {
                for (prop_name, prop_value) in props {
                    let track = tracks
                        .entry(id.clone())
                        .or_default()
                        .entry(prop_name.clone())
                        .or_default();
                    track.push(Keyframe {
                        time: 0.0,
                        value: prop_value.clone(),
                        easing: "linear".to_string(),
                        easing_params: None,
                    });
                }
            }
        }

        // Add keyframes from timeline entries
        for entry in &scene.timeline {
            if let Value::Object(state) = &entry.state {
                for (prop_name, prop_value) in state {
                    let track = tracks
                        .entry(entry.object.clone())
                        .or_default()
                        .entry(prop_name.clone())
                        .or_default();
                    track.push(Keyframe {
                        time: entry.time,
                        value: prop_value.clone(),
                        easing: entry.easing.clone(),
                        easing_params: entry.easing_params.clone(),
                    });
                }
            }
        }

        // Sort keyframes by time — use total_cmp to avoid panicking on NaN
        for object_tracks in tracks.values_mut() {
            for track in object_tracks.values_mut() {
                track.sort_by(|a, b| a.time.total_cmp(&b.time));
            }
        }

        Self {
            duration: scene.canvas.duration,
            fps: scene.canvas.fps,
            tracks,
            overrides: HashMap::new(),
        }
    }

    /// Set an interactive override that takes precedence over keyframes
    /// (used by `tween_to`/`set_property` event actions).
    pub fn override_property(&mut self, object_id: &str, property: &str, value: Value) {
        self.overrides
            .entry(object_id.to_string())
            .or_default()
            .insert(property.to_string(), value);
    }

    /// Evaluate every object's full property state at `time`.
    ///
    /// Builds the `serde_json::Map` for each object directly rather than
    /// filling a `HashMap` and converting it afterwards. The old shape walked
    /// and reallocated every property twice per frame; this walks them once.
    pub fn get_state_at(&self, time: f32) -> HashMap<String, Value> {
        let mut state: HashMap<String, Value> =
            HashMap::with_capacity(self.tracks.len() + self.overrides.len());

        for (obj_id, object_tracks) in &self.tracks {
            let mut props = serde_json::Map::with_capacity(object_tracks.len());
            for (prop_name, track) in object_tracks {
                props.insert(prop_name.clone(), self.evaluate_track(track, time));
            }
            state.insert(obj_id.clone(), Value::Object(props));
        }

        // Interactive overrides take precedence over keyframes.
        for (obj_id, obj_overrides) in &self.overrides {
            match state.get_mut(obj_id) {
                Some(Value::Object(props)) => {
                    for (prop_name, value) in obj_overrides {
                        props.insert(prop_name.clone(), value.clone());
                    }
                }
                _ => {
                    let mut props = serde_json::Map::with_capacity(obj_overrides.len());
                    for (prop_name, value) in obj_overrides {
                        props.insert(prop_name.clone(), value.clone());
                    }
                    state.insert(obj_id.clone(), Value::Object(props));
                }
            }
        }

        state
    }

    /// Evaluate the camera state at `time` (identity when the scene has no
    /// camera block).
    pub fn get_camera_at(&self, time: f32, scene: &Scene) -> lumina_schema::CameraState {
        use lumina_schema::CameraState;

        let default = CameraState {
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
        };
        let camera = match &scene.camera {
            Some(c) => c,
            None => return default,
        };
        if camera.timeline.is_empty() {
            return default;
        }

        let kfs = &camera.timeline;
        if time <= kfs[0].time {
            return kfs[0].state.clone();
        }
        let last = &kfs[kfs.len() - 1];
        if time >= last.time {
            return last.state.clone();
        }

        // Binary search, matching `evaluate_track`. The clamps above put the
        // split point strictly inside the slice.
        let idx = kfs.partition_point(|k| k.time <= time);
        let k0 = &kfs[idx - 1];
        let k1 = &kfs[idx];
        if k1.time <= k0.time {
            return k0.state.clone();
        }

        let t_raw = (time - k0.time) / (k1.time - k0.time);
        // Through `eval_easing`, so a camera keyframe gets the same curve an
        // object property would from the same easing and parameters. It used
        // to use the parameterless lookup, which does not know `cubic_bezier`
        // or `spline` and silently fell through to `linear`.
        let t = crate::easing::eval_easing(&k1.easing, k1.easing_params.as_ref(), t_raw);
        CameraState {
            x: k0.state.x + (k1.state.x - k0.state.x) * t,
            y: k0.state.y + (k1.state.y - k0.state.y) * t,
            zoom: k0.state.zoom + (k1.state.zoom - k0.state.zoom) * t,
        }
    }

    fn evaluate_track(&self, track: &[Keyframe], time: f32) -> Value {
        if track.is_empty() {
            return Value::Null;
        }

        // Clamp to first keyframe if before start
        if time <= track[0].time {
            return track[0].value.clone();
        }

        // Clamp to last keyframe if after end
        if time >= track[track.len() - 1].time {
            return track[track.len() - 1].value.clone();
        }

        // Find the bracketing pair. Tracks are sorted at construction, so this
        // is a binary search rather than the linear scan it used to be — it
        // runs once per property, per object, per frame.
        //
        // The clamps above have already established
        // `track[0].time < time < track[last].time`, so the split point is
        // strictly inside the slice and both indices are in range.
        let idx = track.partition_point(|k| k.time <= time);
        let lower = &track[idx - 1];
        let upper = &track[idx];

        if lower.time == upper.time {
            return lower.value.clone();
        }

        let t = (time - lower.time) / (upper.time - lower.time);
        // Easing is set on the destination keyframe (CSS convention).
        interpolate_value(
            &lower.value,
            &upper.value,
            t,
            &upper.easing,
            upper.easing_params.as_ref(),
        )
    }
}
