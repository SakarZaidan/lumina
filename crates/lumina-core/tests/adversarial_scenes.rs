//! Adversarial scenes: arbitrary JSON through deserialisation, validation, and
//! timeline construction.
//!
//! This is the path a server takes for every request body. The `fuzz/`
//! `scene_json` target explores the same route; this pins the shapes already
//! known to be dangerous and runs them on stable, in CI, on every commit —
//! which is the only way they get run at all, since `cargo-fuzz` needs
//! nightly and CI does not have it.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use lumina_core::{validation::validate_scene_data, SceneGraph, Timeline};
use lumina_schema::Scene;

/// Deserialise, validate, and — if it validated — build the runtime state.
///
/// All of this runs before a renderer sees the scene, so a panic anywhere in
/// it is reachable from a request body.
fn round_trip(json: &str) {
    let Ok(scene) = serde_json::from_str::<Scene>(json) else {
        return; // rejecting malformed input is correct
    };
    let response = validate_scene_data(&scene);
    if response.valid {
        let timeline = Timeline::from_scene(&scene);
        let _ = SceneGraph::from_scene(&scene);
        // Boundaries and beyond: a renderer clamps, but nothing stops a caller
        // asking for a negative or enormous time.
        for t in [-1.0, 0.0, 0.5, 1.0, 1e6, f32::MAX] {
            let _ = timeline.get_state_at(t);
            let _ = timeline.get_camera_at(t, &scene);
        }
    }
}

fn scene_json(objects: &str, timeline: &str) -> String {
    // `r##` rather than `r#`: the background colour contains `"#`, which would
    // otherwise close the raw string.
    format!(
        r##"{{"version":"1.0",
            "meta":{{"title":"t","author":"a","created_at":"2026-01-01T00:00:00Z"}},
            "canvas":{{"width":64,"height":64,"fps":30,"duration":1.0,"background":"#000000"}},
            "objects":{objects},"timeline":{timeline}}}"##
    )
}

#[test]
fn malformed_json_is_rejected_without_panicking() {
    for case in [
        "",
        "{",
        "}",
        "[]",
        "null",
        "0",
        "\"\"",
        "{\"version\":",
        "{\"version\":1}",
        "{\"objects\":{}}",
    ] {
        round_trip(case);
    }
}

#[test]
fn structurally_valid_but_hostile_scenes_terminate() {
    let cases = [
        // A group that contains itself.
        scene_json(
            r#"{"g":{"type":"Group","properties":{"x":0,"y":0,"children":["g"]}}}"#,
            "[]",
        ),
        // Two groups containing each other.
        scene_json(
            r#"{"a":{"type":"Group","properties":{"x":0,"y":0,"children":["b"]}},
                "b":{"type":"Group","properties":{"x":0,"y":0,"children":["a"]}}}"#,
            "[]",
        ),
        // A group naming a child that does not exist.
        scene_json(
            r#"{"g":{"type":"Group","properties":{"x":0,"y":0,"children":["ghost"]}}}"#,
            "[]",
        ),
        // A keyframe for an object that does not exist.
        scene_json(
            "{}",
            r#"[{"time":0.5,"object":"ghost","state":{"cx":1},"easing":"linear"}]"#,
        ),
        // Keyframes at identical, negative, and absurd times.
        scene_json(
            r#"{"c":{"type":"Circle","properties":{"cx":0,"cy":0,"radius":1}}}"#,
            r#"[{"time":0.0,"object":"c","state":{"cx":1},"easing":"linear"},
                {"time":0.0,"object":"c","state":{"cx":2},"easing":"linear"},
                {"time":-5.0,"object":"c","state":{"cx":3},"easing":"linear"}]"#,
        ),
        // Property types that change between keyframes.
        scene_json(
            r#"{"c":{"type":"Circle","properties":{"cx":0,"cy":0,"radius":1}}}"#,
            r#"[{"time":0.5,"object":"c","state":{"cx":"a string"},"easing":"linear"},
                {"time":1.0,"object":"c","state":{"cx":[1,2,3]},"easing":"linear"}]"#,
        ),
        // Empty and mismatched point arrays, which the morphing path pads.
        scene_json(
            r#"{"p":{"type":"Polygon","properties":{"points":[[0,0],[1,1]]}}}"#,
            r#"[{"time":1.0,"object":"p","state":{"points":[]},"easing":"linear"}]"#,
        ),
    ];
    for case in cases {
        round_trip(&case);
    }
}

#[test]
fn a_deep_group_chain_is_rejected_rather_than_overflowing() {
    // A straight chain contains no cycle, so cycle detection alone recursed to
    // the end. This is the shape that overflowed the stack during validation.
    let depth = 10_000;
    let mut objects = String::from("{");
    for i in 0..depth {
        if i > 0 {
            objects.push(',');
        }
        let child = if i + 1 < depth {
            format!("[\"g{}\"]", i + 1)
        } else {
            "[]".to_string()
        };
        objects.push_str(&format!(
            r#""g{i}":{{"type":"Group","properties":{{"x":0,"y":0,"children":{child}}}}}"#
        ));
    }
    objects.push('}');

    let scene: Scene = serde_json::from_str(&scene_json(&objects, "[]")).expect("parses");
    let response = validate_scene_data(&scene);
    assert!(
        response
            .errors
            .iter()
            .any(|e| e.code == "GROUP_NESTING_TOO_DEEP"),
        "a {depth}-deep chain must be rejected, got {:?}",
        response.errors.iter().map(|e| &e.code).collect::<Vec<_>>()
    );
}

#[test]
fn every_easing_name_survives_a_real_scene() {
    // The registry is the contract, so walking it means a newly added easing
    // cannot ship without being evaluated once end to end.
    for name in lumina_core::easing::EASING_NAMES {
        let json = scene_json(
            r#"{"c":{"type":"Circle","properties":{"cx":0,"cy":0,"radius":1}}}"#,
            &format!(r#"[{{"time":1.0,"object":"c","state":{{"cx":100}},"easing":"{name}"}}]"#),
        );
        round_trip(&json);
    }
}
