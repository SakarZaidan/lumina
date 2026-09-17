use crate::fix::{apply_patch, fix_scene, fix_text, patch_text, PatchError, DEFAULT_MAX_ROUNDS};
use serde_json::{json, Value};

fn scene(objects: &Value, timeline: &Value) -> Value {
    let mut scene = json!({
        "version": "1.0",
        "meta": { "title": "t", "author": "a", "created_at": "2026-01-01T00:00:00Z" },
        "canvas": { "width": 64, "height": 64, "fps": 30, "duration": 2.0,
                    "background": "#000000" }
    });
    scene["objects"] = objects.clone();
    scene["timeline"] = timeline.clone();
    scene
}

fn circle() -> Value {
    json!({ "circle": { "type": "Circle", "properties": { "cx": 32, "cy": 32, "radius": 10 } } })
}

#[test]
fn a_misspelled_keyframe_property_is_renamed() {
    let raw = scene(
        &circle(),
        &json!([{ "time": 1.0, "object": "circle", "state": { "raduis": 20 } }]),
    );
    let report = fix_scene(&raw, DEFAULT_MAX_ROUNDS);
    assert!(report.remaining.valid, "{:?}", report.remaining.errors);
    assert_eq!(
        report.scene["timeline"][0]["state"],
        json!({ "radius": 20 })
    );
    assert_eq!(report.applied.len(), 1);
    assert_eq!(report.applied[0].code, "UNKNOWN_PROPERTY");
}

#[test]
fn a_fix_that_exposes_another_is_followed_in_the_next_round() {
    // The entry's properties are only checked once it names a real object, so
    // `raduis` is invisible until round one has fixed `circel`.
    let raw = scene(
        &circle(),
        &json!([{ "time": 1.0, "object": "circel", "state": { "raduis": 20 } }]),
    );
    let report = fix_scene(&raw, DEFAULT_MAX_ROUNDS);
    assert!(report.remaining.valid, "{:?}", report.remaining.errors);
    let rounds: Vec<_> = report
        .applied
        .iter()
        .map(|f| (f.code.as_str(), f.round))
        .collect();
    assert_eq!(rounds, [("UNKNOWN_OBJECT_ID", 1), ("UNKNOWN_PROPERTY", 2)]);
}

#[test]
fn two_misspellings_of_one_property_never_collapse_into_one() {
    // Both are one edit from `radius`. Renaming both would silently throw a
    // value away; the second must stay an error with its value intact.
    let raw = scene(
        &circle(),
        &json!([{ "time": 1.0, "object": "circle", "state": { "raduis": 20, "radus": 30 } }]),
    );
    let report = fix_scene(&raw, DEFAULT_MAX_ROUNDS);
    let state = &report.scene["timeline"][0]["state"];
    assert_eq!(
        state.as_object().map(serde_json::Map::len),
        Some(2),
        "{state}"
    );
    assert!(!report.remaining.valid);
    assert!(report
        .remaining
        .errors
        .iter()
        .any(|e| e.code == "UNKNOWN_PROPERTY"));
}

#[test]
fn a_problem_that_needs_judgement_is_left_alone() {
    let raw = scene(
        &circle(),
        &json!([{ "time": 1.0, "object": "circle", "state": { "radius": "big" } }]),
    );
    let report = fix_scene(&raw, DEFAULT_MAX_ROUNDS);
    assert!(report.applied.is_empty());
    assert_eq!(report.scene, raw);
    assert_eq!(report.remaining.errors[0].code, "PROPERTY_TYPE_MISMATCH");
}

#[test]
fn easings_types_and_references_are_repaired() {
    let raw = scene(
        &json!({
            "axes": { "type": "Axes",
                      "properties": { "x_range": [0, 1], "y_range": [0, 1], "x": 0, "y": 0 } },
            "plot": { "type": "Plot",
                      "properties": { "function_str": "x", "axes_id": "axis" } },
            "dot": { "type": "Cirle", "properties": { "cx": 1, "cy": 1, "radius": 1 } }
        }),
        &json!([{ "time": 1.0, "object": "plot", "state": { "opacity": 1 },
                  "easing": "ease_in_out_quadd" }]),
    );
    let report = fix_scene(&raw, DEFAULT_MAX_ROUNDS);
    assert!(report.remaining.valid, "{:?}", report.remaining.errors);
    assert_eq!(
        report.scene["objects"]["plot"]["properties"]["axes_id"],
        "axes"
    );
    assert_eq!(report.scene["objects"]["dot"]["type"], "Circle");
    assert_eq!(report.scene["timeline"][0]["easing"], "ease_in_out_quad");
}

#[test]
fn an_object_id_needing_pointer_escapes_is_patched_correctly() {
    // `/` and `~` are the two characters a JSON Pointer must escape.
    let raw = scene(
        &json!({ "a/b~c": { "type": "Circle",
                            "properties": { "cx": 1, "cy": 1, "radius": 1, "opacty": 1 } } }),
        &json!([]),
    );
    let report = fix_scene(&raw, DEFAULT_MAX_ROUNDS);
    assert!(report.remaining.valid, "{:?}", report.remaining.errors);
    assert_eq!(report.scene["objects"]["a/b~c"]["properties"]["opacity"], 1);
}

#[test]
fn a_valid_scene_comes_back_unchanged_and_zero_rounds_changes_nothing() {
    let valid = scene(&circle(), &json!([]));
    let report = fix_scene(&valid, DEFAULT_MAX_ROUNDS);
    assert!(report.applied.is_empty() && report.remaining.valid);
    assert_eq!(report.scene, valid);

    let broken = scene(
        &circle(),
        &json!([{ "time": 1.0, "object": "circle", "state": { "raduis": 20 } }]),
    );
    assert!(fix_scene(&broken, 0).applied.is_empty());
}

#[test]
fn a_patch_is_all_or_nothing() {
    let mut doc = json!({ "a": 1, "b": 2 });
    let patch = [
        json!({ "op": "replace", "path": "/a", "value": 10 }),
        json!({ "op": "move", "from": "/b", "path": "/a" }),
    ];
    assert_eq!(
        apply_patch(&mut doc, &patch),
        Err(PatchError::WouldOverwrite("/a".to_string()))
    );
    assert_eq!(
        doc,
        json!({ "a": 1, "b": 2 }),
        "a failed patch changed the document"
    );
}

/// A scene written the way the examples are: compact objects on one line, keys
/// in an authored rather than alphabetical order.
const HAND_WRITTEN: &str = r##"{
  "version": "1.0",
  "meta": { "title": "t", "author": "a", "created_at": "2026-01-01T00:00:00Z" },
  "canvas": { "width": 64, "height": 64, "fps": 30, "duration": 2.0, "background": "#000000" },
  "objects": {
    "circle": { "type": "Circle", "properties": { "radius": 10, "cx": 32, "cy": 32, "opacty": 0.5 } }
  },
  "timeline": [
    { "time": 1.0, "object": "circel", "state": { "raduis": 20 }, "easing": "ease_out_cubicc" }
  ]
}
"##;

#[test]
fn fixing_a_file_changes_only_the_misspelled_words() {
    let report = fix_text(HAND_WRITTEN, DEFAULT_MAX_ROUNDS).expect("json");
    assert!(report.remaining.valid, "{:?}", report.remaining.errors);
    let expected = HAND_WRITTEN
        .replace("\"opacty\"", "\"opacity\"")
        .replace("\"circel\"", "\"circle\"")
        .replace("\"raduis\"", "\"radius\"")
        .replace("\"ease_out_cubicc\"", "\"ease_out_cubic\"");
    assert_eq!(report.text, expected);
}

#[test]
fn fixing_text_and_fixing_the_document_agree() {
    let text = fix_text(HAND_WRITTEN, DEFAULT_MAX_ROUNDS).expect("json");
    let doc: Value = serde_json::from_str(HAND_WRITTEN).expect("json");
    let tree = fix_scene(&doc, DEFAULT_MAX_ROUNDS);
    assert_eq!(
        serde_json::from_str::<Value>(&text.text).expect("json"),
        tree.scene
    );
    assert_eq!(text.applied.len(), tree.applied.len());
}

#[test]
fn keys_written_with_escapes_and_strings_full_of_brackets_are_found() {
    // `\/` is a legal way to write `/`, which a pointer spells `~1`; and
    // braces or quotes inside a string must not be taken for structure.
    let text = r#"{"objects": {"a\/b": {"note": "}{\"][", "opacty": 1}}}"#;
    let patch = [json!({ "op": "move",
                         "from": "/objects/a~1b/opacty", "path": "/objects/a~1b/opacity" })];
    assert_eq!(
        patch_text(text, &patch).expect("patched"),
        r#"{"objects": {"a\/b": {"note": "}{\"][", "opacity": 1}}}"#
    );
}

#[test]
fn an_array_element_is_replaced_in_place() {
    let text = "{\"children\": [ \"a\",\n  \"bb\" , \"c\" ]}";
    let patch = [json!({ "op": "replace", "path": "/children/1", "value": "b" })];
    assert_eq!(
        patch_text(text, &patch).expect("patched"),
        "{\"children\": [ \"a\",\n  \"b\" , \"c\" ]}"
    );
}

#[test]
fn text_patches_refuse_what_they_cannot_do_exactly() {
    let text = r#"{"a": {"x": 1, "y": 2}, "b": {}}"#;
    let onto_existing = [json!({ "op": "move", "from": "/a/x", "path": "/a/y" })];
    assert_eq!(
        patch_text(text, &onto_existing),
        Err(PatchError::WouldOverwrite("/a/y".to_string()))
    );
    let across = [json!({ "op": "move", "from": "/a/x", "path": "/b/x" })];
    assert!(matches!(
        patch_text(text, &across),
        Err(PatchError::UnsupportedOperation(_))
    ));
    let nowhere = [json!({ "op": "replace", "path": "/a/z", "value": 1 })];
    assert_eq!(
        patch_text(text, &nowhere),
        Err(PatchError::NoSuchPath("/a/z".to_string()))
    );
    assert!(fix_text("{ not json", DEFAULT_MAX_ROUNDS).is_err());
}
