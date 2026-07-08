//! Binary entry point for the Lumina HTTP API server. See the library crate
//! docs (`lumina_server`) for endpoints and the current security posture.

use lumina_server::run_server;

#[tokio::main]
async fn main() {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
    run_server().await;
}
