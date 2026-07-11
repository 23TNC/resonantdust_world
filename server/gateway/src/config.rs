//! Gateway configuration, resolved from the environment at startup. One binary
//! serves every env (dev/claude/test/alpha) — the env-specific bits (which HTTP
//! port to listen on, which `index` database to read) come in as vars, mirroring
//! how the world `server` binary reads `SERVER_LISTEN`.

/// Where the gateway's own HTTP API listens. `0.0.0.0` so a published container
/// port reaches it; override with `GATE_LISTEN`.
const DEFAULT_LISTEN: &str = "0.0.0.0:8080";

/// SpacetimeDB host holding the `index` routing DB. The SDK accepts an
/// `http(s)`/`ws(s)` base; default is the local spacetime server.
const DEFAULT_INDEX_URI: &str = "http://127.0.0.1:3000";

/// `index` database name. Per-env DBs are `resonantdust-<env>-index-0`
/// (see `bin/rd`), so the default targets the dev deployment.
const DEFAULT_INDEX_DB: &str = "resonantdust-dev-index-0";

/// Resolved gateway configuration.
#[derive(Debug, Clone)]
pub struct GatewayConfig {
    /// `host:port` the gateway's HTTP API binds to.
    pub listen: String,
    /// SpacetimeDB URI the `index` DB lives on.
    pub index_uri: String,
    /// `index` database name to subscribe to.
    pub index_db: String,
}

impl GatewayConfig {
    /// Read the configuration from the environment, falling back to the local-dev
    /// defaults for any var that isn't set.
    pub fn from_env() -> Self {
        Self {
            listen: env_or("GATE_LISTEN", DEFAULT_LISTEN),
            index_uri: env_or("INDEX_URI", DEFAULT_INDEX_URI),
            index_db: env_or("INDEX_DB", DEFAULT_INDEX_DB),
        }
    }
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}
