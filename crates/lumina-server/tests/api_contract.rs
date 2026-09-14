//! The HTTP surface, exercised end to end through the real middleware stack.
//!
//! `ServiceExt::oneshot` drives the router in-process rather than binding a
//! port: a test that needs a free port is a test that fails on a busy machine,
//! and racing a spawned server task makes failures look like flakes.
//!
//! These tests are about the *contract* — status codes, the error envelope,
//! and who is allowed through — rather than about rendering, which the
//! renderer's own suites cover.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use luminafx_server::{build_router_with, ServerConfig};
use tower::ServiceExt;

/// A config with the limiter and auth off, for tests about something else.
/// Each test that cares turns on exactly the thing it is testing.
fn open() -> ServerConfig {
    ServerConfig {
        rate_limit_per_minute: 0,
        ..ServerConfig::default()
    }
}

async fn send(config: &ServerConfig, request: Request<Body>) -> (StatusCode, serde_json::Value) {
    let response = build_router_with(config)
        .oneshot(request)
        .await
        .expect("router responded");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, json)
}

fn post(path: &str, body: &serde_json::Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("request")
}

fn get(path: &str) -> Request<Body> {
    Request::builder()
        .uri(path)
        .body(Body::empty())
        .expect("request")
}

fn minimal_scene() -> serde_json::Value {
    serde_json::json!({
        "version": "1.0",
        "meta": { "title": "t", "author": "a", "created_at": "2026-01-01T00:00:00Z" },
        "canvas": { "width": 64, "height": 64, "fps": 30, "duration": 1.0,
                    "background": "#000000" },
        "objects": {},
        "timeline": []
    })
}

#[tokio::test]
async fn health_answers_without_credentials() {
    // A health check that needs a token is a health check that stops being
    // run, so it is exempt on purpose — and that exemption needs a test, or a
    // future middleware change will quietly break every deployment's probe.
    let config = ServerConfig {
        api_token: Some("a-token-with-real-entropy".into()),
        ..open()
    };
    let (status, _) = send(&config, get("/health")).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn a_protected_endpoint_refuses_an_anonymous_caller() {
    let config = ServerConfig {
        api_token: Some("a-token-with-real-entropy".into()),
        ..open()
    };
    let (status, body) = send(&config, get("/schema")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["code"], "UNAUTHORIZED");
    assert!(
        body["fix_suggestion"].is_string(),
        "an error an agent has to act on must say how"
    );
}

#[tokio::test]
async fn a_wrong_token_is_refused_and_the_right_one_is_not() {
    let config = ServerConfig {
        api_token: Some("a-token-with-real-entropy".into()),
        ..open()
    };
    let wrong = Request::builder()
        .uri("/schema")
        .header("authorization", "Bearer not-the-token")
        .body(Body::empty())
        .expect("request");
    let (status, _) = send(&config, wrong).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let right = Request::builder()
        .uri("/schema")
        .header("authorization", "Bearer a-token-with-real-entropy")
        .body(Body::empty())
        .expect("request");
    let (status, _) = send(&config, right).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn no_token_configured_means_no_authentication() {
    // The development default. It must stay working, or every contributor's
    // first `cargo run -p luminafx-server` fails.
    let (status, _) = send(&open(), get("/schema")).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn every_error_uses_the_same_envelope() {
    // The reason this matters: the HTTP surface exists so an agent can drive
    // it in a loop — send, read what is wrong, fix, resend. `/validate`
    // answered with `code`/`path`/`message`/`fix_suggestion` and everything
    // else answered with a bare string an agent would have to parse with a
    // regex.
    let cases = [
        (
            "/patch",
            serde_json::json!({ "scene": minimal_scene(), "patch": [{ "bad": 1 }] }),
        ),
        (
            "/scene_patch",
            serde_json::json!({ "scene": minimal_scene(),
                                "patch": { "op": "no_such_op" } }),
        ),
    ];
    for (path, body) in cases {
        let (status, json) = send(&open(), post(path, &body)).await;
        assert!(status.is_client_error(), "{path} returned {status}");
        assert!(
            json["code"].is_string(),
            "{path} answered without a machine-readable code: {json}"
        );
        assert!(
            json["message"].is_string(),
            "{path} answered without a message: {json}"
        );
        assert_eq!(
            json["status"],
            status.as_u16(),
            "{path}'s envelope disagrees with its status line"
        );
    }
}

#[tokio::test]
async fn the_rate_limiter_admits_its_allowance_then_refuses() {
    let config = ServerConfig {
        rate_limit_per_minute: 3,
        ..ServerConfig::default()
    };
    // One router, so the limiter's state persists across the requests — a
    // fresh router per request would reset the window and the test would pass
    // against a limiter that does nothing.
    let app = build_router_with(&config);
    for i in 0..3 {
        let response = app
            .clone()
            .oneshot(get("/health"))
            .await
            .expect("responded");
        assert_eq!(response.status(), StatusCode::OK, "request {i} was refused");
    }
    let response = app.oneshot(get("/health")).await.expect("responded");
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn an_oversized_body_is_refused_before_it_is_parsed() {
    // 8 MiB cap. The point is that this is rejected by a layer rather than by
    // a handler, so the JSON is never parsed and the memory never allocated.
    let huge = "x".repeat(9 * 1024 * 1024);
    let request = Request::builder()
        .method("POST")
        .uri("/validate")
        .header("content-type", "application/json")
        .body(Body::from(format!("{{\"junk\":\"{huge}\"}}")))
        .expect("request");
    let (status, _) = send(&open(), request).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn cross_origin_is_closed_unless_it_is_opened() {
    // This shipped as `CorsLayer::permissive()`, which let any website a
    // developer happened to visit drive their local render server.
    let response = build_router_with(&open())
        .oneshot(
            Request::builder()
                .uri("/health")
                .header("origin", "https://evil.example")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("responded");
    assert!(
        response
            .headers()
            .get("access-control-allow-origin")
            .is_none(),
        "an unlisted origin was told it is allowed"
    );
}

#[tokio::test]
async fn a_configured_origin_is_allowed() {
    let config = ServerConfig {
        cors_origins: vec!["https://studio.example".into()],
        ..open()
    };
    let response = build_router_with(&config)
        .oneshot(
            Request::builder()
                .uri("/health")
                .header("origin", "https://studio.example")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("responded");
    assert_eq!(
        response
            .headers()
            .get("access-control-allow-origin")
            .and_then(|v| v.to_str().ok()),
        Some("https://studio.example")
    );
}

#[tokio::test]
async fn validate_still_answers_with_its_own_shape() {
    // The envelope work must not have changed `/validate`, whose response is a
    // ValidationResponse rather than an ApiError and is already a documented
    // contract.
    let (status, json) = send(&open(), post("/validate", &minimal_scene())).await;
    assert_eq!(status, StatusCode::OK);
    assert!(json["valid"].is_boolean(), "got {json}");
    assert!(json["errors"].is_array());
}

#[tokio::test]
async fn a_malformed_body_gets_the_envelope_too() {
    // Converting the handlers was not enough. `axum`'s `Json` extractor
    // rejects a bad body *before* any handler runs, with its own plain-text
    // response — so the endpoint whose job is to explain what is wrong with
    // your JSON answered the most common mistake in prose. This is the case
    // that found it.
    let request = Request::builder()
        .method("POST")
        .uri("/validate")
        .header("content-type", "application/json")
        .body(Body::from("{\"version\": \"1.0\","))
        .expect("request");
    let (status, json) = send(&open(), request).await;
    assert!(status.is_client_error(), "got {status}");
    assert_eq!(json["code"], "MALFORMED_JSON");
    assert!(json["fix_suggestion"].is_string());
}

#[tokio::test]
async fn a_well_formed_body_of_the_wrong_shape_says_so_distinctly() {
    // Valid JSON, wrong document. Distinguishing this from a syntax error is
    // the difference between "fix your braces" and "read the schema", and an
    // agent branches on the code rather than the prose.
    let (status, json) = send(
        &open(),
        post("/validate", &serde_json::json!({ "nope": 1 })),
    )
    .await;
    assert!(status.is_client_error(), "got {status}");
    assert_eq!(json["code"], "SCHEMA_MISMATCH");
}

#[tokio::test]
async fn a_missing_content_type_is_named_rather_than_guessed_at() {
    let request = Request::builder()
        .method("POST")
        .uri("/validate")
        .body(Body::from(minimal_scene().to_string()))
        .expect("request");
    let (status, json) = send(&open(), request).await;
    assert!(status.is_client_error(), "got {status}");
    assert_eq!(json["code"], "MISSING_CONTENT_TYPE");
}
