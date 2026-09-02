//! Model Context Protocol server for the Lumina animation engine.
//!
//! Exposes the engine as MCP tools, so any MCP-capable agent can discover the
//! scene format, validate a document, repair it, and render it — without
//! anybody writing glue code for that particular agent.
//!
//! # Why this exists alongside the HTTP server
//!
//! They are the same capabilities through different doors, and the doors are
//! not interchangeable. HTTP suits a service someone deploys; MCP suits a tool
//! an agent runs beside itself, with no port, no network, and no credentials —
//! stdin and stdout. The engine is a pure function from scene to pixels, so
//! neither door has to own any state, and both can answer from the same code.
//!
//! Both speak the same error vocabulary. An agent that has learned
//! `SCHEMA_MISMATCH` from one knows it in the other.
//!
//! # Transport
//!
//! Newline-delimited JSON-RPC 2.0 on stdio, which is what MCP clients spawn a
//! server as. Anything the server wants to say to a human goes to **stderr**:
//! stdout is the protocol channel, and one stray `println!` corrupts the
//! stream in a way that presents as the client hanging.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// JSON-RPC message shapes.
pub mod protocol;
/// The tools this server exposes.
pub mod tools;

use std::io::{BufRead, Write};

use serde_json::{json, Value};

use protocol::{Request, Response, INVALID_PARAMS, METHOD_NOT_FOUND};

/// Read requests from `input` and write responses to `output` until the input
/// ends.
///
/// # Errors
///
/// Returns an error only if the output stream fails; a malformed *request* is
/// answered with a JSON-RPC error rather than ending the session, because a
/// client that sends one bad message should not lose the ones after it.
pub fn serve<R: BufRead, W: Write>(input: R, mut output: W) -> std::io::Result<()> {
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let request: Request = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                // No id to correlate against, so answer with null per
                // JSON-RPC. Dropping it silently would leave a client that
                // sent one malformed frame waiting forever.
                write_response(
                    &mut output,
                    &Response::err(
                        Value::Null,
                        protocol::INVALID_PARAMS,
                        format!("could not parse the request: {e}"),
                        None,
                    ),
                )?;
                continue;
            }
        };

        // A notification has no id and must not be answered at all. Answering
        // one is a protocol error some clients treat as fatal.
        let Some(id) = request.id.clone() else {
            log::debug!("notification: {}", request.method);
            continue;
        };

        let response = handle(&request, id);
        write_response(&mut output, &response)?;
    }
    Ok(())
}

fn write_response<W: Write>(output: &mut W, response: &Response) -> std::io::Result<()> {
    let line = serde_json::to_string(response)
        .unwrap_or_else(|e| format!(r#"{{"jsonrpc":"2.0","id":null,"error":{{"code":-32603,"message":"could not serialise the response: {e}"}}}}"#));
    writeln!(output, "{line}")?;
    // Flushing every message is the whole contract of a stdio transport: the
    // client is blocked reading a line, and a buffered response looks exactly
    // like a server that has hung.
    output.flush()
}

/// Answer one request.
fn handle(request: &Request, id: Value) -> Response {
    match request.method.as_str() {
        "initialize" => Response::ok(
            id,
            json!({
                "protocolVersion": protocol::PROTOCOL_VERSION,
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "lumina", "version": env!("CARGO_PKG_VERSION") },
                "instructions":
                    "Lumina renders declarative JSON scenes to video. Call lumina_objects \
                     first to learn the object types, write a scene, then lumina_validate \
                     before lumina_render — validation is microseconds and rendering is \
                     seconds. Validation errors carry a fix_suggestion you can apply directly."
            }),
        ),
        "tools/list" => Response::ok(id, json!({ "tools": tools::descriptors() })),
        "tools/call" => {
            let Some(name) = request.params.get("name").and_then(Value::as_str) else {
                return Response::err(id, INVALID_PARAMS, "tools/call needs a `name`", None);
            };
            let args = request
                .params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let result = tools::call(name, &args);
            // MCP carries tool failures inside a successful response, with
            // `isError` set. That is deliberate in the protocol and worth
            // honouring: a model sees a failed tool as content it can reason
            // about and retry, rather than as a transport error it cannot.
            Response::ok(
                id,
                json!({
                    "content": [{
                        "type": "text",
                        "text": serde_json::to_string_pretty(&result.value)
                            .unwrap_or_else(|_| result.value.to_string()),
                    }],
                    "isError": result.is_error,
                }),
            )
        }
        "ping" => Response::ok(id, json!({})),
        other => Response::err(
            id,
            METHOD_NOT_FOUND,
            format!("no method `{other}`"),
            Some(json!({ "supported": ["initialize", "tools/list", "tools/call", "ping"] })),
        ),
    }
}

/// Serve on the process's own stdin and stdout.
///
/// # Errors
///
/// Returns an error if stdout fails.
pub fn serve_stdio() -> std::io::Result<()> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    serve(stdin.lock(), stdout.lock())
}

#[cfg(test)]
mod tests;
