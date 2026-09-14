# Events & Interactivity

Scenes are not just videos — in the WASM player (and any host that embeds the
engine) objects can react to input. Interactivity is declared in the scene
(`events`), dispatched through an event bus, and resolved against the
timeline. Everything stays deterministic: an event changes playback state or
property overrides, and rendering remains a pure function of time + state.

## Declaring events

Each entry binds one object and one trigger to one action:

```json
"events": [
  { "object": "play_button", "trigger": "click",
    "action": { "type": "play_from", "value": 0.0 } },
  { "object": "node_3", "trigger": "click",
    "action": { "type": "show_tooltip", "text": "Hidden layer, ReLU" } }
]
```

`trigger` is a free-form string matched exactly; the host decides what
gestures produce which triggers (the JS SDK maps canvas clicks through
`hit_test` to `click` on the topmost hit object).

## Actions

| `type` | Fields | Effect |
|---|---|---|
| `jump_to_time` | `value` | Seek the playhead (seconds). |
| `play_from` | `value` | Seek and start playback. |
| `pause` | — | Pause playback. |
| `set_property` | `target`, `property`, `value` | Override a property immediately. |
| `tween_to` | `target`, `property`, `value`, `duration`, `easing` | Animate a property to a value from the current playhead. |
| `show_tooltip` | `text` | Ask the host to display a transient overlay. |
| `emit_custom` | `event_name`, `payload` | Send a named event with payload to the host application. |

In `emit_custom` payloads, `$drag.*` placeholders (e.g. `"$drag.from"`,
`"$drag.to"`) are substituted from the incoming event's payload at dispatch
time — useful for wiring drag gestures back into application logic.

## The event bus

`luminafx_core::EventBus` owns a `PlaybackState` and dispatches host events:

- The host constructs an `Event { object_id, trigger, payload }` (usually from
  `hit_test`) and calls `process_event`.
- Every declared entry matching that object + trigger fires.
- The returned `EventOutcome { actions, current_time, playing, emitted }`
  tells the host what to do: update its clock, apply overrides, show
  tooltips, forward emitted events.

In the browser this is wrapped by `LuminaEngine.process_event` /
`LuminaEngine.hit_test(x, y, time)`; hit-testing is geometry-aware for all 17
object types (polygon ray-casting, segment distance for lines and béziers,
recursive group transforms) and respects z-order.

## Scene patching

For programmatic editing — AI loops, editors, live-coding — the engine offers
semantic patch operations that understand the scene's structure (unlike raw
RFC-6902 JSON Patch, which is also available):

| Op | Effect |
|---|---|
| `add_object` / `remove_object` | Insert or delete an object; removal cascades to its timeline entries, events, and group memberships. |
| `update_property` | Change an object's initial property. |
| `add_keyframe` / `update_keyframe` / `remove_keyframe` | Edit timeline entries for one object + time. |
| `add_event` / `remove_event` | Edit interactivity declarations. |
| `update_canvas` | Change canvas dimensions/fps/duration/background. |

Apply them in-process via `luminafx_core::scene_patch::apply_patch`, or over
HTTP with `POST /scene_patch` — the server applies the patch and re-validates
the scene in one round trip, returning structured errors with
`fix_suggestion`s on failure (see the
[AI Integration Cookbook](./ai-integration.md)).
