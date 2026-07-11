//! Client configuration — chiefly *which gateway* to ask for a world server.
//!
//! The client's only static endpoint is the gateway's HTTP base; everything else
//! (the world-server WS url, the data shard) is discovered at runtime from the
//! gateway's reply. Resolution order, most specific first:
//!   1. `CLIENT_GATEWAY_URL` — an explicit base, e.g. `http://127.0.0.1:9473`.
//!   2. `RD_GATEWAY` — exported by the `bin/rd` dev CLI; the canonical
//!      client-facing gateway for the active profile.
//!   3. the per-env default below, keyed by `CLIENT_ENV` / `RD_ENV` (else `dev`).

/// Per-env gateway host:port, mirroring `bin/lib/common.sh`'s client-facing
/// `RD_GATEWAY_PORT` (9473/9474/9475 local; alpha on lightsail). Kept in step
/// with the `rd` CLI — that's the source of truth for what port the gateway is
/// actually published on.
fn gateway_url_for_env(env: &str) -> String {
    match env {
        "claude" => "http://localhost:9474".to_string(),
        "test" => "http://localhost:9475".to_string(),
        // The lightsail gateway isn't fully up yet and is plain HTTP, like the
        // lightsail spacetime server — flip to https once a TLS proxy lands.
        "alpha" => "http://gateway.resonantdust.com:9473".to_string(),
        // `dev` and any unknown env fall back to the user's local gateway.
        _ => "http://localhost:9473".to_string(),
    }
}

/// Deployment env tag, unified on `RD_ENV` (shared with `bin/rd`) with
/// `CLIENT_ENV` as the per-tool override. Precedence: `CLIENT_ENV` > `RD_ENV` >
/// `dev`.
fn env_name() -> String {
    std::env::var("CLIENT_ENV")
        .or_else(|_| std::env::var("RD_ENV"))
        .unwrap_or_else(|_| "dev".to_string())
}

/// Resolved client configuration.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// The gateway's HTTP base (`http(s)://host:port`, no trailing `/server`).
    /// The client issues `GET {gateway_url}/server` to acquire a world server.
    pub gateway_url: String,
}

impl ClientConfig {
    /// Read the configuration from the environment (see the module docs for the
    /// resolution order).
    pub fn from_env() -> Self {
        let gateway_url = std::env::var("CLIENT_GATEWAY_URL")
            .or_else(|_| std::env::var("RD_GATEWAY"))
            .unwrap_or_else(|_| gateway_url_for_env(&env_name()));
        Self { gateway_url }
    }

    /// Point the client at an explicit gateway base — for embedding hosts that
    /// resolve the endpoint themselves rather than from the environment.
    pub fn for_gateway(gateway_url: impl Into<String>) -> Self {
        Self {
            gateway_url: gateway_url.into(),
        }
    }
}
