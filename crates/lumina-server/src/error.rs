//! One error shape for every endpoint.
//!
//! `/validate` already answered with `code` / `path` / `message` /
//! `fix_suggestion`, and everything else answered with a bare string:
//!
//! ```text
//! POST /validate  →  {"code":"UNKNOWN_EASING","path":"$.timeline[0].easing",…}
//! POST /patch     →  Invalid JSON Patch: missing field `op` at line 1 column 9
//! ```
//!
//! That gap matters more here than in most APIs. The whole point of the HTTP
//! surface is that an agent can drive it in a loop — send a scene, read what
//! is wrong, fix it, send it again — and half the responses were prose it
//! would have to parse with a regex. Every handler now answers with
//! [`ApiError`], whose fields are the ones `/validate` already used.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

/// A machine-readable error, shaped like `lumina_core`'s validation errors.
#[derive(Debug, Clone, Serialize)]
pub struct ApiError {
    /// Stable, machine-readable identifier, e.g. `INVALID_JSON_PATCH`. Callers
    /// branch on this; the message is for humans and may be reworded.
    pub code: String,
    /// What went wrong, in a sentence.
    pub message: String,
    /// JSON path into the offending document, when there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// What to do about it. Present wherever the answer is knowable — this is
    /// the field an agent acts on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix_suggestion: Option<String>,
    /// The status this error is returned with. Serialised so a caller reading
    /// a logged body still knows what the client saw.
    pub status: u16,
}

impl ApiError {
    /// An error with a code, a message, and the status to return it with.
    pub fn new(status: StatusCode, code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
            path: None,
            fix_suggestion: None,
            status: status.as_u16(),
        }
    }

    /// Attach the JSON path the error refers to.
    #[must_use]
    pub fn at(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    /// Attach the remedy.
    #[must_use]
    pub fn fix(mut self, suggestion: impl Into<String>) -> Self {
        self.fix_suggestion = Some(suggestion.into());
        self
    }

    /// 400: the request itself is malformed.
    pub fn bad_request(code: &str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, code, message)
    }

    /// 422: the request parsed but describes something impossible.
    pub fn unprocessable(code: &str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNPROCESSABLE_ENTITY, code, message)
    }

    /// 500: our fault.
    ///
    /// The message is what the client sees, so it must not carry filesystem
    /// paths, and the caller is expected to log the detail separately.
    pub fn internal(code: &str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, code, message)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (status, Json(self)).into_response()
    }
}

/// `Json`, but rejections answer with [`ApiError`] like everything else.
///
/// Converting the handlers was not enough. `axum`'s own `Json` extractor
/// rejects a malformed body *before* any handler runs, with its own plain-text
/// response — so the endpoint that exists to tell an agent what is wrong with
/// its JSON answered the most common mistake in prose:
///
/// ```text
/// Failed to parse the request body as JSON: expected `,` or `}` at line 1 column 9
/// ```
///
/// Every route takes this instead, so the envelope covers the whole surface
/// rather than only the parts a handler reached.
pub struct ApiJson<T>(pub T);

#[axum::async_trait]
impl<T, S> axum::extract::FromRequest<S> for ApiJson<T>
where
    T: serde::de::DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(req: axum::extract::Request, state: &S) -> Result<Self, Self::Rejection> {
        use axum::extract::rejection::JsonRejection;
        match Json::<T>::from_request(req, state).await {
            Ok(Json(value)) => Ok(Self(value)),
            Err(rejection) => {
                let (code, fix) = match &rejection {
                    JsonRejection::JsonDataError(_) => (
                        "SCHEMA_MISMATCH",
                        "The body is valid JSON but not the shape this endpoint expects. \
                         GET /schema for the scene shape, or GET /objects for object properties.",
                    ),
                    JsonRejection::JsonSyntaxError(_) => (
                        "MALFORMED_JSON",
                        "The body is not valid JSON. Check for a trailing comma or an \
                         unquoted key.",
                    ),
                    JsonRejection::MissingJsonContentType(_) => (
                        "MISSING_CONTENT_TYPE",
                        "Send `Content-Type: application/json`.",
                    ),
                    JsonRejection::BytesRejection(_) => (
                        "BODY_TOO_LARGE",
                        "The request body exceeds the server's limit. Reduce the scene, or \
                         raise the limit on the server.",
                    ),
                    // `JsonRejection` is non-exhaustive, so a future axum
                    // release can add a variant. Answering with the envelope
                    // and a generic code is better than failing to compile or
                    // falling back to prose.
                    _ => (
                        "INVALID_REQUEST_BODY",
                        "The request body could not be read. GET /schema for the expected shape.",
                    ),
                };
                Err(
                    ApiError::new(rejection.status(), code, rejection.body_text())
                        .at("$")
                        .fix(fix),
                )
            }
        }
    }
}
