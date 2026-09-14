//! Fuzz the whole front door: arbitrary bytes to a validated scene.
//!
//! This is the input a server accepts from the network and the CLI reads from
//! disk. Deserialisation must reject malformed input rather than panic, and
//! validation must terminate — the resource bounds it enforces are only
//! meaningful if reaching them is always possible.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let Ok(scene) = serde_json::from_str::<luminafx_schema::Scene>(text) else {
        return; // malformed input is expected; rejecting it is correct
    };
    // A scene that parsed must validate without panicking, whatever it says.
    let response = luminafx_core::validation::validate_scene_data(&scene);

    // Anything that validates must also build a timeline and scene graph:
    // those run before any renderer sees the scene, so a panic there is
    // reachable from the same input.
    if response.valid {
        let _ = luminafx_core::Timeline::from_scene(&scene);
        let _ = luminafx_core::SceneGraph::from_scene(&scene);
    }
});
