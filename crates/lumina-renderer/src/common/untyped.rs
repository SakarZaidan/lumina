//! The untyped property map the draw code was written against.
//!
//! RFC-0002 Stage 2 hands the renderer resolved, typed objects, but the draw
//! branches still read `state["radius"]`-style JSON. They move to typed field
//! access one object type at a time; until a branch has moved, it reads this
//! map, built from the typed object. The module goes when the last one does.

use luminafx_schema::Object;
use serde_json::Value;

/// `object`'s properties as JSON — what `Timeline::get_state_at` used to hand
/// the renderer for it.
///
/// Built from an object `Timeline::resolve_at` produced, this is the same map
/// the timeline built for any scene that validates. `luminafx-core`'s
/// `resolving_agrees_with_the_untyped_state_on_every_shipped_scene` holds the
/// two to that, which is why moving the renderer onto typed objects does not
/// change a pixel.
pub(crate) fn state_of(object: &Object) -> Value {
    match serde_json::to_value(object) {
        Ok(Value::Object(mut tagged)) => tagged.remove("properties").unwrap_or(Value::Null),
        _ => Value::Null,
    }
}
