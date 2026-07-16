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

/// Where the DSL content tree (`content/{data,visual,biome}/*.rd`) lives, relative
/// to the server's working directory (`/workspace` in the container, so the repo's
/// `content/`). Override with `RD_CONTENT_DIR`. Worldgen reads this to map tile
/// names to the `def_id`s it packs into a zone, re-reading it on the content poll.
const DEFAULT_CONTENT_DIR: &str = "content";

/// Default content re-poll interval (seconds) — how often worldgen re-reads the
/// content tree and hot-reloads on a change. `RD_CONTENT_POLL_SECS`; `0` disables.
const DEFAULT_CONTENT_POLL_SECS: u64 = 10;

/// Default `server_id` registered in the index when `SERVER_ID` is unset — the
/// single-server dev case.
const DEFAULT_SERVER_ID: u16 = 0;
/// Default client-facing WebSocket URL advertised to the gateway when
/// `SERVER_PUBLIC_URL` is unset. Must be reachable *from the client*, so it's
/// the host-facing published port (dev local: `ws://localhost:8473/ws`), not the
/// in-container listen addr. Matches the dev row in `content/servers/dev`; other
/// envs pass their own via `SERVER_PUBLIC_URL`.
const DEFAULT_PUBLIC_URL: &str = "ws://localhost:8473/ws";

/// Where the server reads the DSL corpus it SERVES at `/content` (distinct from
/// worldgen, which always reads `content_dir` off disk). `disk` (default) serves
/// the bind-mounted `content_dir`; `r2` pulls the corpus from the asset bucket
/// (deployed). Any other value falls back to `disk`. `CONTENT_SOURCE`.
const DEFAULT_CONTENT_SOURCE: &str = "disk";

/// Where the server reads texture assets it serves at `/textures`. `disk`
/// (default) reads the bind-mounted `textures/`; `r2` streams from the bucket,
/// reusing the R2 credentials. `TEXTURE_SOURCE`.
const DEFAULT_TEXTURE_SOURCE: &str = "disk";

/// Default local texture directory (relative to the server's `/workspace` cwd,
/// where compose bind-mounts the repo's `textures/`). Read-only. `TEXTURE_DIR`.
const DEFAULT_TEXTURE_DIR: &str = "textures";

/// Default subdir (under the system temp dir) for the derived-preview cache —
/// deliberately NOT under the read-only texture mount, so the server has a
/// writable path. Ephemeral: a miss just re-derives. `TEXTURE_CACHE_DIR`.
const DEFAULT_TEXTURE_CACHE_SUBDIR: &str = "resonantdust-texture-cache";

/// R2 asset-bucket defaults — the bucket + this game's namespace, mirroring
/// `bin/dsl`. Shared by the content (`<prefix>/content/`) and texture
/// (`<prefix>/textures/`) R2 sources. Keys have no default (empty ⇒ that R2
/// source fails at load / per request).
const DEFAULT_R2_ENDPOINT: &str = "https://0d47d810e5fcfd344f83d3e2d80c62f8.r2.cloudflarestorage.com";
const DEFAULT_R2_BUCKET: &str = "resonantdust-assets";
const DEFAULT_R2_REGION: &str = "auto";
const DEFAULT_R2_PREFIX: &str = "resonantdust_world";

/// R2 access parameters shared by the content + texture R2 sources. Its `Debug`
/// redacts the keys so a `{cfg:?}` never leaks them.
#[derive(Clone)]
pub struct R2Settings {
    pub endpoint: String,
    pub bucket: String,
    pub region: String,
    pub prefix: String,
    pub access_key: String,
    pub secret_key: String,
}

impl std::fmt::Debug for R2Settings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let redact = |s: &str| if s.is_empty() { "\"\"".to_string() } else { format!("set({} chars)", s.len()) };
        f.debug_struct("R2Settings")
            .field("endpoint", &self.endpoint)
            .field("bucket", &self.bucket)
            .field("region", &self.region)
            .field("prefix", &self.prefix)
            .field("access_key", &redact(&self.access_key))
            .field("secret_key", &redact(&self.secret_key))
            .finish()
    }
}

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
    /// How often (seconds) to re-read `content_dir` and hot-reload worldgen on a
    /// change. `0` disables the poll (load once at startup). `RD_CONTENT_POLL_SECS`.
    /// The `/content` serving store polls on this same interval.
    pub content_poll_secs: u64,
    /// `disk` | `r2` — the source the `/content` route serves from.
    pub content_source: String,
    /// `disk` | `r2` — the source the `/textures` routes serve from.
    pub texture_source: String,
    /// Local texture directory (for `texture_source = disk`) — the read-only master tree.
    pub texture_dir: String,
    /// Writable directory the server caches derived previews in (kept apart from
    /// the read-only `texture_dir`).
    pub texture_cache_dir: String,
    /// R2 credentials/location shared by the content + texture R2 sources.
    pub r2: R2Settings,
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
            content_poll_secs: std::env::var("RD_CONTENT_POLL_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_CONTENT_POLL_SECS),
            content_source: env_or("CONTENT_SOURCE", DEFAULT_CONTENT_SOURCE),
            texture_source: env_or("TEXTURE_SOURCE", DEFAULT_TEXTURE_SOURCE),
            texture_dir: env_or("TEXTURE_DIR", DEFAULT_TEXTURE_DIR),
            texture_cache_dir: std::env::var("TEXTURE_CACHE_DIR").unwrap_or_else(|_| {
                std::env::temp_dir().join(DEFAULT_TEXTURE_CACHE_SUBDIR).to_string_lossy().into_owned()
            }),
            r2: R2Settings {
                endpoint: env_or("R2_ENDPOINT", DEFAULT_R2_ENDPOINT),
                bucket: env_or("R2_BUCKET", DEFAULT_R2_BUCKET),
                region: env_or("R2_REGION", DEFAULT_R2_REGION),
                prefix: env_or("R2_PREFIX", DEFAULT_R2_PREFIX),
                access_key: env_or("R2_ACCESS_KEY_ID", ""),
                secret_key: env_or("R2_SECRET_ACCESS_KEY", ""),
            },
        }
    }

    /// The `index` routing-directory database (region→shard→endpoint and
    /// player→server→endpoint). Single instance today (shard 0).
    pub fn index_db(&self) -> String {
        format!("resonantdust-{}-index-0", self.env)
    }

    /// The single `players` auth DB (account record + profile). One instance
    /// today; login resolves a name → `player_id` + `player_shard_reference` here.
    pub fn players_db(&self) -> String {
        format!("resonantdust-{}-players-0", self.env)
    }

    /// The `event_shard` DB — the event queue + client-visible settled `event` log. The edge queues
    /// client intents here (`queue`) and subscribes to `event` per zone. Single instance today.
    pub fn event_shard_db(&self) -> String {
        format!("resonantdust-{}-event-shard-0", self.env)
    }

    /// The `data_shard` DB — the composition slots + client-visible `state`. The edge subscribes to
    /// `state` per zone. Single instance today; multi-shard routing (via `index`) is future work.
    pub fn data_shard_db(&self) -> String {
        format!("resonantdust-{}-data-shard-0", self.env)
    }

    /// Fallback shard database name for a region whose `region_shards` entry is
    /// missing — single-shard deployments run with no index rows seeded, so an
    /// unrouted region defaults to shard 0 on *this* SpacetimeDB server. Mirrors
    /// the old gateway's "default to shard 0" posture.
    ///
    /// The unified `shard` module deploys to the `zone` db family
    /// (`rd_db_for zone 0`): SpacetimeDB db names are DNS-like, so the db is named
    /// `zone-0` (matching `bin/rd`'s `RD_DB`). A single shard now carries both a
    /// zone's terrain and its loose objects through one tick pipeline, so there is
    /// no separate object-shard db family anymore.
    pub fn default_shard_db(&self) -> String {
        format!("resonantdust-{}-zone-0", self.env)
    }

    // The standalone cold-tiles / cold-things DBs are retired — a zone's cold objects live in
    // the shard's own `cold` table now (spacetime-rewrite S7, divergence #3).
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}
