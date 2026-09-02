//! Authentication and rate limiting.
//!
//! Both are deliberately small and in-process. A shared secret and a token
//! bucket are the right size for a single-node render server; anything
//! distributed belongs behind a reverse proxy that already does it better, and
//! pretending otherwise would be a worse answer than saying so.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::extract::{ConnectInfo, State};
use axum::http::{Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::error::ApiError;

/// Compare two secrets without leaking their contents through timing.
///
/// `==` on `str` returns as soon as it finds a differing byte, so the time it
/// takes reveals how long a shared prefix is — enough, over many requests, to
/// recover a token a byte at a time. This looks at every byte regardless.
///
/// The length is compared first and *not* in constant time. That leaks the
/// token's length, which is not a secret worth protecting: an attacker who
/// knows only the length has learned nothing they could not have guessed.
fn secret_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Require `Authorization: Bearer <token>` when a token is configured.
///
/// `/health` is exempt. A health check that needs credentials is a health
/// check that stops being run, and the endpoint reveals nothing: it returns a
/// fixed string and touches no scene data.
pub async fn require_auth(
    State(expected): State<Option<String>>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let Some(expected) = expected else {
        return next.run(request).await;
    };
    if request.uri().path() == "/health" {
        return next.run(request).await;
    }

    let presented = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::trim);

    match presented {
        Some(token) if secret_eq(token, &expected) => next.run(request).await,
        _ => ApiError::new(
            StatusCode::UNAUTHORIZED,
            "UNAUTHORIZED",
            "this endpoint requires a bearer token",
        )
        .fix("Send `Authorization: Bearer <token>`, matching the server's LUMINA_API_TOKEN.")
        .into_response(),
    }
}

/// A fixed-window request counter, keyed by client address.
///
/// A fixed window rather than a sliding one: it admits up to twice the limit
/// across a window boundary, which for a render server is irrelevant — the
/// limit exists to stop one caller monopolising the CPU, not to meter billing
/// — and it costs one integer per client instead of a timestamp list.
pub struct RateLimiter {
    per_minute: u32,
    window: Mutex<Window>,
}

struct Window {
    started: Instant,
    counts: HashMap<IpAddr, u32>,
}

impl RateLimiter {
    /// A limiter admitting `per_minute` requests per client. Zero disables it.
    #[must_use]
    pub fn new(per_minute: u32) -> Self {
        Self {
            per_minute,
            window: Mutex::new(Window {
                started: Instant::now(),
                counts: HashMap::new(),
            }),
        }
    }

    /// Record a request from `who`, and say whether it is allowed.
    ///
    /// The map is cleared when the window rolls, which is also what bounds its
    /// memory: a flood from many addresses cannot grow it for longer than a
    /// minute, so the limiter cannot become the exhaustion it exists to
    /// prevent.
    pub fn check(&self, who: IpAddr) -> bool {
        if self.per_minute == 0 {
            return true;
        }
        let Ok(mut w) = self.window.lock() else {
            // A poisoned lock means another thread panicked while holding it.
            // Failing open is the right call for a rate limiter: the
            // alternative is that one panic takes the whole server offline.
            return true;
        };
        if w.started.elapsed() >= Duration::from_secs(60) {
            w.started = Instant::now();
            w.counts.clear();
        }
        let n = w.counts.entry(who).or_insert(0);
        *n += 1;
        *n <= self.per_minute
    }
}

/// Reject requests from a client that has exceeded its allowance.
///
/// `ConnectInfo` is optional, and that is load-bearing rather than defensive.
/// The extractor *rejects* when a request carries no connection information,
/// which would turn every such request into a 500 — and requests without it
/// are not exotic: any in-process call (the integration tests drive the router
/// directly) and some proxy setups have none. A limiter that answers 500 is
/// worse than the flood it prevents.
///
/// When the address is unknown the request is counted against a single shared
/// bucket. That is the conservative reading: unknown callers are rate-limited
/// together rather than exempted.
pub async fn rate_limit(
    State(limiter): State<std::sync::Arc<RateLimiter>>,
    connect_info: Option<ConnectInfo<std::net::SocketAddr>>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let who = connect_info.map_or(IpAddr::from([0, 0, 0, 0]), |ConnectInfo(a)| a.ip());
    if limiter.check(who) {
        return next.run(request).await;
    }
    ApiError::new(
        StatusCode::TOO_MANY_REQUESTS,
        "RATE_LIMITED",
        "too many requests from this address",
    )
    .fix("Wait for the current minute to elapse, or raise LUMINA_RATE_LIMIT on the server.")
    .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_token_matches_itself_and_nothing_else() {
        assert!(secret_eq("hunter2hunter2hu", "hunter2hunter2hu"));
        assert!(!secret_eq("hunter2hunter2hu", "hunter2hunter2hv"));
        assert!(!secret_eq("hunter2", "hunter2hunter2hu"));
        assert!(!secret_eq("", "x"));
        assert!(secret_eq("", ""));
    }

    #[test]
    fn the_comparison_reads_every_byte() {
        // The property that makes it constant-time: a mismatch in the first
        // byte and a mismatch in the last must both be rejected, and the
        // implementation must not have returned early on either. Timing cannot
        // be asserted portably, so this pins the observable behaviour and the
        // fold in `secret_eq` is what provides the rest.
        let base = "aaaaaaaaaaaaaaaa";
        for i in 0..base.len() {
            let mut other: Vec<u8> = base.bytes().collect();
            other[i] = b'b';
            let other = String::from_utf8(other).expect("ascii");
            assert!(!secret_eq(base, &other), "differing at {i} was accepted");
        }
    }

    #[test]
    fn the_limiter_admits_its_allowance_then_stops() {
        let limiter = RateLimiter::new(3);
        let who: IpAddr = "10.0.0.1".parse().expect("addr");
        assert!(limiter.check(who));
        assert!(limiter.check(who));
        assert!(limiter.check(who));
        assert!(!limiter.check(who), "the fourth request was admitted");
    }

    #[test]
    fn clients_are_counted_separately() {
        // Otherwise one busy caller silences everyone else, which is the
        // denial of service the limiter exists to prevent.
        let limiter = RateLimiter::new(2);
        let a: IpAddr = "10.0.0.1".parse().expect("addr");
        let b: IpAddr = "10.0.0.2".parse().expect("addr");
        assert!(limiter.check(a));
        assert!(limiter.check(a));
        assert!(!limiter.check(a));
        assert!(limiter.check(b), "a second client was blocked by the first");
    }

    #[test]
    fn zero_disables_the_limiter() {
        let limiter = RateLimiter::new(0);
        let who: IpAddr = "10.0.0.1".parse().expect("addr");
        for _ in 0..1000 {
            assert!(limiter.check(who));
        }
    }
}
