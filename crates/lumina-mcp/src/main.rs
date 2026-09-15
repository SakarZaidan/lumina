//! MCP server binary. Speaks newline-delimited JSON-RPC 2.0 on stdio.
//!
//! Register it with an MCP client by pointing at this executable; it takes no
//! arguments and needs no port.

fn main() {
    // stderr, never stdout: stdout is the protocol channel, and a log line on
    // it corrupts the stream in a way that presents to the client as a hang.
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .target(env_logger::Target::Stderr)
        .init();

    if let Err(e) = luminafx_mcp::serve_stdio() {
        log::error!("mcp server stopped: {e}");
        std::process::exit(1);
    }
}
