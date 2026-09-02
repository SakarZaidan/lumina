//! The JSON-RPC 2.0 envelope MCP speaks, and nothing more.
//!
//! Hand-rolled rather than pulled from a crate. The protocol surface an MCP
//! server needs is four message shapes and three methods; a dependency for
//! that would be more code to audit than the code it replaces, and this crate
//! is one an agent runs on a developer's machine with access to their
//! filesystem — the smallest honest dependency footprint is the right one.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Protocol revision this server implements.
///
/// Sent back verbatim in `initialize`. A client asking for a different
/// revision still gets this one and can decide whether to proceed, which is
/// what the specification asks for: the server states what it speaks rather
/// than guessing what the client wanted.
pub const PROTOCOL_VERSION: &str = "2024-11-05";

/// An incoming JSON-RPC request or notification.
///
/// `id` is absent for notifications, and that distinction is load-bearing: a
/// notification must not be answered at all, and answering one is a protocol
/// error that some clients treat as fatal.
#[derive(Debug, Deserialize)]
pub struct Request {
    /// Always `"2.0"`.
    #[serde(default)]
    pub jsonrpc: String,
    /// Correlation id. Absent means this is a notification.
    #[serde(default)]
    pub id: Option<Value>,
    /// Method name.
    pub method: String,
    /// Method parameters.
    #[serde(default)]
    pub params: Value,
}

/// An outgoing JSON-RPC response.
#[derive(Debug, Serialize)]
pub struct Response {
    /// Always `"2.0"`.
    pub jsonrpc: &'static str,
    /// The id of the request being answered.
    pub id: Value,
    /// Present on success.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// Present on failure. Never both.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

/// A JSON-RPC error object.
#[derive(Debug, Serialize)]
pub struct RpcError {
    /// JSON-RPC error code.
    pub code: i32,
    /// Human-readable summary.
    pub message: String,
    /// Structured detail, where there is any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// The method does not exist. (JSON-RPC reserved code.)
pub const METHOD_NOT_FOUND: i32 = -32601;
/// The parameters are wrong for the method. (JSON-RPC reserved code.)
pub const INVALID_PARAMS: i32 = -32602;
/// Anything else that went wrong on our side. (JSON-RPC reserved code.)
pub const INTERNAL_ERROR: i32 = -32603;

impl Response {
    /// A successful response to `id`.
    pub fn ok(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        }
    }

    /// A failed response to `id`.
    pub fn err(id: Value, code: i32, message: impl Into<String>, data: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(RpcError {
                code,
                message: message.into(),
                data,
            }),
        }
    }
}
