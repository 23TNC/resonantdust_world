//! Static deployment config: the SpacetimeDB server URI + environment, plus the
//! naming convention that turns a module (and the index's routing tier) into a
//! concrete database name.
//!
//! This answers *what a control-plane database is called* (`index`, `players`).
//! *Which* shard a given region lives on — and the URL + db_name of that shard —
//! is answered at runtime by the `index` database (`region_shards` → `shards`),
//! not by this config; see [`crate::index`].

/// SpacetimeDB server the control-plane DBs live on. On the `resonantdust`
/// docker network the spacetime container is reachable as `start`; override
/// with `SERVER_STDB_URI`.
const DEFAULT_STDB_URI: &str = "http://start:3000";
const DEFAULT_ENV: &str = "dev";

/// Where the DSL content tree (`content/{data,visual}/*.rd`) lives, relative to
/// the server's working directory (`/workspace` in the container, so the repo's
/// `content/`). Override with `RD_CONTENT_DIR`. Worldgen reads this once at
/// startup to map tile names to the `def_id`s it packs into a zone.
const DEFAULT_CONTENT_DIR: &str = "content";

/// Default `server_id` registered in the index when `SERVER_ID` is unset — the
/// single-server dev case.
const DEFAULT_SERVER_ID: u16 = 0;
/// Default client-facing WebSocket URL advertised to the gateway when
/// `SERVER_PUBLIC_URL` is unset. Must be reachable *from the client*, so it's
/// the host-facing published port (dev local: `ws://localhost:8473/ws`), not the
/// in-container listen addr. Matches the dev row in `content/servers/dev`; other
/// envs pass their own via `SERVER_PUBLIC_URL`.
const DEFAULT_PUBLIC_URL: &str = "ws://localhost:8473/ws";

/// Resolved server configuration.
#[derive(Clone, Debug)]
pub struct ServerConfig {
    /// SpacetimeDB server URI the `index` and `players` control-plane DBs live
    /// on. Data shards may live on *other* SpacetimeDB servers — each `shards`
    /// row in the index carries its own `url`, so a shard connection uses that,
    /// not this.
    pub uri: String,
    /// Deployment environment tag (`dev` / `claude` / `test` / …). Selects the
    /// control-plane database names.
    pub env: String,
    /// This server's id in the index `servers` table. The gateway pins players to
    /// a `server_id`; one server per id. Single-server dev uses `0`.
    pub server_id: u16,
    /// The client-facing WebSocket URL the gateway hands out for this server.
    /// Advertised verbatim to clients, so it must be reachable *from the client*
    /// (the host-facing published port), not the in-container listen addr.
    pub public_url: String,
    /// Path to the DSL content tree worldgen loads at startup (see
    /// [`DEFAULT_CONTENT_DIR`]).
    pub content_dir: String,
}

impl ServerConfig {
    pub fn from_env() -> Self {
        // Environment selection unifies on `RD_ENV` (shared with `bin/rd`), with
        // `SERVER_ENV` as a per-tool override. Precedence: SERVER_ENV > RD_ENV >
        // `dev`.
        let env = std::env::var("SERVER_ENV")
            .or_else(|_| std::env::var("RD_ENV"))
            .unwrap_or_else(|_| DEFAULT_ENV.to_string());
        let server_id = std::env::var("SERVER_ID")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_SERVER_ID);
        Self {
            uri: std::env::var("SERVER_STDB_URI").unwrap_or_else(|_| DEFAULT_STDB_URI.to_string()),
            env,
            server_id,
            public_url: std::env::var("SERVER_PUBLIC_URL")
                .unwrap_or_else(|_| DEFAULT_PUBLIC_URL.to_string()),
            content_dir: std::env::var("RD_CONTENT_DIR")
                .unwrap_or_else(|_| DEFAULT_CONTENT_DIR.to_string()),
        }
    }

    /// The `index` routing-directory database (region→shard→endpoint and
    /// player→server→endpoint). Single instance today (shard 0).
    pub fn index_db(&self) -> String {
        format!("resonantdust-{}-index-0", self.env)
    }

    /// The single `players` auth DB (account record + profile). One instance
    /// today; login resolves a name → `player_id` + `data_shard` here.
    pub fn players_db(&self) -> String {
        format!("resonantdust-{}-players-0", self.env)
    }

    /// Fallback shard database name for a region whose `region_shards` entry is
    /// missing — single-shard deployments run with no index rows seeded, so an
    /// unrouted region defaults to shard 0 on *this* SpacetimeDB server. Mirrors
    /// the old gateway's "default to shard 0" posture.
    ///
    /// The `region_shard` *module* deploys to the `shard` db family
    /// (`rd_db_for shard 0`): SpacetimeDB db names are DNS-like and reject the
    /// underscore in `region_shard`, so the db is named `shard-0` (matching
    /// `bin/rd`'s `RD_DB`). The module crate keeps its descriptive name.
    pub fn default_shard_db(&self) -> String {
        format!("resonantdust-{}-shard-0", self.env)
    }
}
