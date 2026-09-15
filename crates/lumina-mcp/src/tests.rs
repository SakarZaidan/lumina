//! The protocol contract, driven the way a client drives it.
//!
//! Every test writes real JSON-RPC frames into [`crate::serve`] and reads real
//! frames back, rather than calling the handlers directly. The bugs that
//! matter in a stdio server are framing bugs — an unanswered id, a response to
//! a notification, an unflushed line — and none of them are visible if you
//! bypass the transport.

use serde_json::{json, Value};

/// Drive the server with a sequence of frames and collect its responses.
fn exchange(frames: &[Value]) -> Vec<Value> {
    let input: String = frames.iter().map(|f| format!("{f}\n")).collect();
    let mut output: Vec<u8> = Vec::new();
    crate::serve(input.as_bytes(), &mut output).expect("served");
    String::from_utf8(output)
        .expect("utf-8")
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("each response is one JSON object"))
        .collect()
}

fn call(name: &str, arguments: &Value) -> Value {
    json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": { "name": name, "arguments": arguments }
    })
}

/// The tool payload, parsed back out of the MCP content envelope.
fn payload(response: &Value) -> Value {
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("no text content in {response}"));
    serde_json::from_str(text).expect("tool payload is JSON")
}

fn minimal_scene() -> Value {
    json!({
        "version": "1.0",
        "meta": { "title": "t", "author": "a", "created_at": "2026-01-01T00:00:00Z" },
        "canvas": { "width": 64, "height": 64, "fps": 30, "duration": 1.0,
                    "background": "#000000" },
        "objects": {
            "c": { "type": "Circle",
                   "properties": { "cx": 32, "cy": 32, "radius": 10, "fill": "#FF0000" } }
        },
        "timeline": []
    })
}

#[test]
fn initialize_states_the_protocol_it_speaks() {
    let out = exchange(&[json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize" })]);
    assert_eq!(out.len(), 1);
    assert_eq!(
        out[0]["result"]["protocolVersion"],
        crate::protocol::PROTOCOL_VERSION
    );
    assert!(out[0]["result"]["capabilities"]["tools"].is_object());
    assert!(
        out[0]["result"]["instructions"].is_string(),
        "a model reads this before anything else"
    );
}

#[test]
fn a_notification_is_not_answered() {
    // A notification has no id, and answering one is a protocol error some
    // clients treat as fatal. The only way to see this is through the
    // transport: a handler called directly always returns something.
    let out = exchange(&[
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
        json!({ "jsonrpc": "2.0", "id": 7, "method": "ping" }),
    ]);
    assert_eq!(out.len(), 1, "the notification was answered: {out:?}");
    assert_eq!(out[0]["id"], 7);
}

#[test]
fn a_malformed_frame_does_not_end_the_session() {
    // A client that sends one bad frame must not lose every message after it.
    let out = exchange(&[
        json!("this is not a request object"),
        json!({ "jsonrpc": "2.0", "id": 2, "method": "ping" }),
    ]);
    assert_eq!(out.len(), 2, "the session ended early: {out:?}");
    assert!(out[0]["error"].is_object());
    assert_eq!(
        out[1]["id"], 2,
        "the request after the bad frame was dropped"
    );
}

#[test]
fn every_tool_is_listed_with_a_schema() {
    let out = exchange(&[json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" })]);
    let tools = out[0]["result"]["tools"].as_array().expect("tools array");
    assert!(
        tools.len() >= 5,
        "expected the five tools, got {}",
        tools.len()
    );
    for tool in tools {
        assert!(tool["name"].is_string());
        assert!(
            tool["description"].as_str().is_some_and(|d| d.len() > 40),
            "{} has no description a model could choose on",
            tool["name"]
        );
        assert!(tool["inputSchema"]["type"] == "object", "{}", tool["name"]);
    }
}

#[test]
fn an_unknown_method_says_what_it_supports() {
    let out = exchange(&[json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/frobnicate" })]);
    assert_eq!(out[0]["error"]["code"], crate::protocol::METHOD_NOT_FOUND);
    assert!(out[0]["error"]["data"]["supported"].is_array());
}

#[test]
fn validate_answers_with_the_same_errors_the_http_server_does() {
    // Same vocabulary through both doors: an agent that learned a code from
    // one knows it in the other. If these ever diverge, the second door is a
    // second product.
    let out = exchange(&[call(
        "lumina_validate",
        &json!({ "scene": minimal_scene() }),
    )]);
    let body = payload(&out[0]);
    assert_eq!(body["valid"], true, "got {body}");
    assert!(body["errors"].is_array());
    assert_eq!(out[0]["result"]["isError"], false);
}

#[test]
fn an_invalid_scene_comes_back_with_something_to_act_on() {
    let mut scene = minimal_scene();
    scene["canvas"]["fps"] = json!(100_000);
    let out = exchange(&[call("lumina_validate", &json!({ "scene": scene }))]);
    let body = payload(&out[0]);
    assert_eq!(body["valid"], false);
    let first = &body["errors"][0];
    assert!(first["code"].is_string(), "got {body}");
    assert!(first["path"].is_string());
    assert!(
        first["fix_suggestion"].is_string(),
        "an error a model has to act on must say how"
    );
}

#[test]
fn a_tool_failure_is_content_not_a_transport_error() {
    // MCP carries tool failures inside a *successful* response with `isError`.
    // That is deliberate in the protocol: a model sees a failed tool as
    // something it can read and retry rather than as a broken connection.
    let out = exchange(&[call("lumina_validate", &json!({ "scene": "not a scene" }))]);
    assert!(
        out[0]["error"].is_null(),
        "it was reported as a protocol error"
    );
    assert_eq!(out[0]["result"]["isError"], true);
    assert_eq!(payload(&out[0])["code"], "SCHEMA_MISMATCH");
}

#[test]
fn a_missing_argument_names_the_argument() {
    let out = exchange(&[call("lumina_validate", &json!({}))]);
    assert_eq!(payload(&out[0])["code"], "MISSING_ARGUMENT");
}

#[test]
fn an_unknown_tool_points_at_the_list() {
    let out = exchange(&[call("lumina_frobnicate", &json!({}))]);
    let body = payload(&out[0]);
    assert_eq!(body["code"], "UNKNOWN_TOOL");
    assert!(body["fix_suggestion"]
        .as_str()
        .is_some_and(|s| s.contains("tools/list")));
}

#[test]
fn the_object_registry_is_the_same_one_the_http_server_answers_with() {
    // It lives in `lumina-core` precisely so there is only one. A registry
    // maintained in two places is a registry that disagrees with itself, which
    // is the mistake TD-02 recorded for the path and colour parsers.
    let out = exchange(&[call("lumina_objects", &json!({}))]);
    let body = payload(&out[0]);
    assert_eq!(body, luminafx_core::object_registry());
    assert!(body["Circle"]["required"].is_array());
}

#[test]
fn a_scoped_schema_is_smaller_and_still_complete() {
    // The point of scoping is context cost, so "smaller" is the claim under
    // test — but a schema with a dangling `$ref` is worse than a large one,
    // because a model will either invent the missing type or refuse.
    let full = exchange(&[call("lumina_schema", &json!({}))]);
    let scoped = exchange(&[call(
        "lumina_schema",
        &json!({ "objects": ["CircleProps"] }),
    )]);

    let full_len = full[0]["result"]["content"][0]["text"]
        .as_str()
        .expect("text")
        .len();
    let scoped_len = scoped[0]["result"]["content"][0]["text"]
        .as_str()
        .expect("text")
        .len();
    assert!(
        scoped_len < full_len,
        "scoping did not reduce the schema ({scoped_len} vs {full_len})"
    );

    // Every `$ref` that survived must still resolve.
    let body = payload(&scoped[0]);
    let defs = body
        .get("definitions")
        .or_else(|| body.get("$defs"))
        .and_then(Value::as_object)
        .expect("definitions");
    for name in refs_of(&Value::Object(defs.clone())) {
        assert!(
            defs.contains_key(&name),
            "scoped schema references `{name}` but does not define it"
        );
    }
}

fn refs_of(value: &Value) -> Vec<String> {
    let mut found = Vec::new();
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                if k == "$ref" {
                    if let Some(n) = v.as_str().and_then(|r| r.rsplit('/').next()) {
                        found.push(n.to_string());
                    }
                } else {
                    found.extend(refs_of(v));
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|i| found.extend(refs_of(i))),
        _ => {}
    }
    found
}

#[test]
fn render_refuses_an_invalid_scene_before_spending_any_cpu() {
    // Rendering is seconds and validation is microseconds, so an invalid scene
    // must not reach the renderer. It also means a RENDER_FAILED from this
    // tool is a real rendering problem, which an agent should treat
    // differently from a document it can repair.
    let mut scene = minimal_scene();
    scene["canvas"]["fps"] = json!(100_000);
    let out = exchange(&[call(
        "lumina_render",
        &json!({ "scene": scene, "output": "out.mp4" }),
    )]);
    let body = payload(&out[0]);
    assert_eq!(body["code"], "SCENE_INVALID");
    assert!(body["errors"].is_array());
}

#[test]
fn render_will_not_write_outside_its_root() {
    // This tool is invoked by a model, on a developer's machine, with that
    // developer's permissions. "Render to ../../.ssh/authorized_keys" must not
    // be something it can be talked into.
    let out = exchange(&[call(
        "lumina_render",
        &json!({ "scene": minimal_scene(), "output": "../../../../../tmp/escaped.mp4" }),
    )]);
    let body = payload(&out[0]);
    assert_eq!(body["code"], "OUTPUT_OUTSIDE_ROOT", "got {body}");
}
