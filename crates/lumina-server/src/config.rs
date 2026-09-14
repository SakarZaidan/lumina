//! Server configuration, read from the environment.
//!
//! Every setting has a default that is safe to run unattended, which is a
//! change: the server used to bind `0.0.0.0` and allow any origin, so starting
//! it on a laptop on a café network published a render endpoint to everyone on
//! that network. Defaults now assume the operator wants a local development
//! server, and opening it up is an explicit act.

use std::net::SocketAddr;
use std::time::Duration;

/// How the server binds, who may call it, and how hard.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Address to bind. `LUMINA_BIND`, default `127.0.0.1:3000`.
    pub bind: SocketAddr,
    /// Shared secret required in `Authorization: Bearer …`.
    /// `LUMINA_API_TOKEN`; unset means no authentication.
    pub api_token: Option<String>,
    /// Origins allowed to make cross-origin requests.
    /// `LUMINA_CORS_ORIGINS`, comma-separated; unset means none.
    pub cors_origins: Vec<String>,
    /// Requests allowed per client per minute. `LUMINA_RATE_LIMIT`, default 60;
    /// `0` disables the limiter.
    pub rate_limit_per_minute: u32,
    /// How long a single request may run before it is abandoned.
    /// `LUMINA_REQUEST_TIMEOUT_SECS`, default 300.
    pub request_timeout: Duration,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            // Loopback, not `0.0.0.0`. Binding every interface by default
            // meant the difference between "I started the dev server" and "I
            // published a render endpoint to the local network" was invisible.
            bind: SocketAddr::from(([127, 0, 0, 1], 3000)),
            api_token: None,
            cors_origins: Vec::new(),
            // A render is seconds of CPU, so the useful limit is low. Sixty a
            // minute is far above any interactive use and far below what it
            // takes to saturate the machine.
            rate_limit_per_minute: 60,
            request_timeout: Duration::from_secs(300),
        }
    }
}

impl ServerConfig {
    /// Read the configuration from the environment, falling back to
    /// [`ServerConfig::default`] for anything unset.
    ///
    /// A malformed value is a hard error rather than a silent fallback: a
    /// typo in `LUMINA_BIND` that quietly reverted to the default would be a
    /// server listening somewhere its operator did not intend.
    ///
    /// # Errors
    ///
    /// Returns a message naming the variable when a value cannot be parsed.
    pub fn from_env() -> Result<Self, String> {
        let mut cfg = Self::default();

        if let Some(v) = env_var("LUMINA_BIND") {
            cfg.bind = v
                .parse()
                .map_err(|e| format!("LUMINA_BIND is not a socket address ({v}): {e}"))?;
        }
        cfg.api_token = env_var("LUMINA_API_TOKEN");
        if let Some(v) = env_var("LUMINA_CORS_ORIGINS") {
            cfg.cors_origins = v
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect();
        }
        if let Some(v) = env_var("LUMINA_RATE_LIMIT") {
            cfg.rate_limit_per_minute = v
                .parse()
                .map_err(|e| format!("LUMINA_RATE_LIMIT is not a number ({v}): {e}"))?;
        }
        if let Some(v) = env_var("LUMINA_REQUEST_TIMEOUT_SECS") {
            let secs: u64 = v
                .parse()
                .map_err(|e| format!("LUMINA_REQUEST_TIMEOUT_SECS is not a number ({v}): {e}"))?;
            cfg.request_timeout = Duration::from_secs(secs);
        }
        Ok(cfg)
    }

    /// Warn about settings that are fine for development and not for exposure.
    ///
    /// Deliberately a warning rather than a refusal. Running unauthenticated on
    /// loopback is the normal development case, and a server that refused to
    /// start would be worked around with a token nobody rotates.
    pub fn warn_about_exposure(&self) {
        let public = !self.bind.ip().is_loopback();
        if public && self.api_token.is_none() {
            log::warn!(
                "listening on {} with no LUMINA_API_TOKEN — every caller on this network can \
                 spend this machine's CPU rendering",
                self.bind
            );
        }
        if public && self.rate_limit_per_minute == 0 {
            log::warn!("listening on {} with the rate limiter disabled", self.bind);
        }
        if self.api_token.as_ref().is_some_and(|t| t.len() < 16) {
            log::warn!(
                "LUMINA_API_TOKEN is shorter than 16 characters — it is a shared secret \
                        on a public endpoint, so give it real entropy"
            );
        }
    }
}

/// A trimmed environment variable, or `None` when unset or empty.
///
/// Empty is treated as unset because `LUMINA_API_TOKEN=` in a shell script or
/// compose file reads as "no token" to a human and would otherwise configure a
/// server whose password is the empty string.
fn env_var(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}
