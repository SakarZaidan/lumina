//! Binary entry point for the Lumina HTTP API server. See the library crate
//! docs (`luminafx_server`) for endpoints and the current security posture.

use luminafx_server::run_server;

#[tokio::main]
async fn main() {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
    if let Err(e) = run_server().await {
        log::error!("server failed: {e}");
        std::process::exit(1);
    }
}
