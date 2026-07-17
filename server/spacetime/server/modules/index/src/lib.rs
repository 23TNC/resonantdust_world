//! Index database — the routing directory the **gateway** (entrypoint) and the
//! **servers** (state-authoritative game processes) consult. Two parallel
//! two-step LUTs in one DB:
//!
//! ```text
//! region_shards:  region_id (u32) -> shard_id (u16)   data routing
//! shards:         shard_id  (u16) -> { url, db_name }
//!
//! player_servers: player_id (u32) -> server_id (u16)  player routing
//! servers:        server_id (u16) -> { url }
//! ```
//!
//! **Topology.** A client connects to a *gateway* (entrypoint) and requests a
//! server; the gateway picks a *server* and returns it; the client connects there
//! and that server becomes authoritative for the player's live state. Servers
//! read the authoritative *data* from the SpacetimeDB *shards* backing them
//! (the `zone_shard` module DBs today; an `object_shard` will join them). So a
//! region resolves to a shard+endpoint (which DB
//! holds its zones), and a player resolves to a server+endpoint (which process
//! owns its session).
//!
//! **Why store the player→server pin.** On reconnect the gateway re-pins the
//! player to the server already holding their state, so it can *stream the delta*
//! rather than stand everything back up and re-subscribe. [`index_gc`] releases
//! pins to dead servers (so they're reallocated) and drops idle players (so
//! capacity frees) — see the TTLs below.
//!
//! Both assignments are current-value (latest wins) — plain primary-key tables,
//! no `valid_at`. **Clients never read this; the gateway/servers do.** Reducers
//! trust their args — **authorization is the gateway's / ops' job**.
//!
//! ## ids
//! - `zone_id`   u32 = `region_x:8 | region_y:8 | surface:8 | zone_x:4 | zone_y:4`
//! - `region_id` u32 = `region_x:8 | region_y:8 | surface:8 | reserved:8`
//!
//! A zone's region is its `zone_id` with the low byte cleared — see [`region_of`].

use spacetimedb::{reducer, table, ReducerContext, ScheduleAt, Table, TimeDuration};

// ── region routing (data) ────────────────────────────────────────────────────

// The `zone_id ↔ region_id` mask lives in the shared `resonantdust_codec`
// (reached via the `../shared:/workspace/shared` bind-mount), so this routing
// module and the native server resolve regions with the same code — they used
// to carry a duplicated mask with a "keep the two in lockstep" comment.
pub use resonantdust_codec::packed::{region_of, REGION_ID_MASK};

/// Step one: which shard holds a given region. One row per assigned region.
#[table(accessor = region_shards, public)]
pub struct RegionShard {
    /// `region_id` = `realm:8 | region:8 | reserved:16` — a `zone_id` under [`REGION_ID_MASK`],
    /// each level an `x:4 | y:4` nibble pair. Authoritative shape: `docs/VARIABLES.md`.
    /// (Was documented here as `region_x:8 | region_y:8 | surface:8 | reserved:8` — the
    /// pre-0.2.3 layout, whose `surface` z-axis is retired. Comment only; the code takes
    /// the mask from the codec and was always correct.)
    #[primary_key]
    pub region_id: u32,
    /// The shard holding this region — keys into [`Shard`].
    pub shard_id: u16,
}

/// Step two: where a data shard physically lives. One row per shard.
#[table(accessor = shards, public)]
pub struct Shard {
    #[primary_key]
    pub shard_id: u16,
    /// SpacetimeDB server URL a game server connects to for this shard.
    pub url: String,
    /// Database name on that server holding the shard's regions.
    pub db_name: String,
}

/// Assign (or reassign) `region_id` to `shard_id`. Upsert — latest wins.
#[reducer]
pub fn assign_region(ctx: &ReducerContext, region_id: u32, shard_id: u16) -> Result<(), String> {
    // delete-then-insert upsert (delete is a no-op when absent), the codebase
    // idiom for single-row keys.
    ctx.db.region_shards().region_id().delete(region_id);
    ctx.db.region_shards().insert(RegionShard { region_id, shard_id });
    Ok(())
}

/// Drop a region's assignment (e.g. before relocating it).
#[reducer]
pub fn unassign_region(ctx: &ReducerContext, region_id: u32) -> Result<(), String> {
    ctx.db.region_shards().region_id().delete(region_id);
    Ok(())
}

/// Register (or update) a data shard's endpoint. Upsert — latest wins.
#[reducer]
pub fn set_shard(
    ctx: &ReducerContext,
    shard_id: u16,
    url: String,
    db_name: String,
) -> Result<(), String> {
    ctx.db.shards().shard_id().delete(shard_id);
    ctx.db.shards().insert(Shard { shard_id, url, db_name });
    Ok(())
}

/// Remove a data shard's endpoint row.
#[reducer]
pub fn remove_shard(ctx: &ReducerContext, shard_id: u16) -> Result<(), String> {
    ctx.db.shards().shard_id().delete(shard_id);
    Ok(())
}

// ── player routing (sessions) ────────────────────────────────────────────────

/// Step two (server tier): a state-authoritative game server's endpoint +
/// liveness. `last_seen_ms` is refreshed by heartbeat; [`index_gc`] reaps a
/// server (and releases its players) once it goes stale.
#[table(accessor = servers, public)]
pub struct Server {
    #[primary_key]
    pub server_id: u16,
    /// URL clients connect to for this server (a process endpoint — no db_name).
    pub url: String,
    /// Last heartbeat, gateway/server-resolved ms. See [`SERVER_TTL_MS`].
    pub last_seen_ms: u64,
}

/// Step one (server tier): which server owns a player's live session. The pin
/// the gateway re-uses on reconnect. `last_seen_ms` tracks player activity;
/// [`index_gc`] releases an idle pin after [`PLAYER_TTL_MS`].
#[table(accessor = player_servers, public)]
pub struct PlayerServer {
    /// Logical player id (from the `players` auth module; the gateway maps the
    /// connection's identity → player_id).
    #[primary_key]
    pub player_id: u32,
    /// The server holding this player's session — keys into [`Server`].
    pub server_id: u16,
    /// Last player activity, gateway/server-resolved ms.
    pub last_seen_ms: u64,
}

/// Register (or refresh) a server's endpoint and stamp its heartbeat. Upsert.
#[reducer]
pub fn set_server(
    ctx: &ReducerContext,
    server_id: u16,
    url: String,
    now_ms: u64,
) -> Result<(), String> {
    ctx.db.servers().server_id().delete(server_id);
    ctx.db.servers().insert(Server { server_id, url, last_seen_ms: now_ms });
    Ok(())
}

/// Refresh a registered server's heartbeat without resending its url. No-op if
/// the server isn't registered (it must `set_server` first).
#[reducer]
pub fn heartbeat_server(ctx: &ReducerContext, server_id: u16, now_ms: u64) -> Result<(), String> {
    if let Some(mut s) = ctx.db.servers().server_id().find(server_id) {
        s.last_seen_ms = now_ms;
        ctx.db.servers().server_id().delete(server_id);
        ctx.db.servers().insert(s);
    }
    Ok(())
}

/// Deregister a server and release every player pinned to it (so the gateway
/// reallocates them on next connect).
#[reducer]
pub fn remove_server(ctx: &ReducerContext, server_id: u16) -> Result<(), String> {
    ctx.db.servers().server_id().delete(server_id);
    release_players_of(ctx, server_id);
    Ok(())
}

/// Pin `player_id` to `server_id` and stamp activity. Upsert — the gateway calls
/// this when it allocates a server to a player.
#[reducer]
pub fn assign_player(
    ctx: &ReducerContext,
    player_id: u32,
    server_id: u16,
    now_ms: u64,
) -> Result<(), String> {
    ctx.db.player_servers().player_id().delete(player_id);
    ctx.db.player_servers().insert(PlayerServer {
        player_id,
        server_id,
        last_seen_ms: now_ms,
    });
    Ok(())
}

/// Refresh a player's activity timestamp (heartbeat) without changing its pin.
/// No-op if the player isn't pinned.
#[reducer]
pub fn touch_player(ctx: &ReducerContext, player_id: u32, now_ms: u64) -> Result<(), String> {
    if let Some(mut p) = ctx.db.player_servers().player_id().find(player_id) {
        p.last_seen_ms = now_ms;
        ctx.db.player_servers().player_id().delete(player_id);
        ctx.db.player_servers().insert(p);
    }
    Ok(())
}

/// Release a player's pin (e.g. clean logout).
#[reducer]
pub fn release_player(ctx: &ReducerContext, player_id: u32) -> Result<(), String> {
    ctx.db.player_servers().player_id().delete(player_id);
    Ok(())
}

/// Delete every player pinned to `server_id` (cascade on server removal / GC).
fn release_players_of(ctx: &ReducerContext, server_id: u16) {
    let pks: Vec<u32> = ctx
        .db
        .player_servers()
        .iter()
        .filter(|p| p.server_id == server_id)
        .map(|p| p.player_id)
        .collect();
    for pk in pks {
        ctx.db.player_servers().player_id().delete(pk);
    }
}

// ── cold-shard routing (position → cold shard) ───────────────────────────────

/// Which cold shard serves a `(cold type, region)`. The fresh, **region-keyed** cold router (the
/// dead `region_shards`/`shards` above was `zone_id`-keyed). The edge resolves a cold subscription by
/// `position → macro_position → region_reference`, then `(type, region) →` this row `→` endpoint. One
/// row per assigned `(type, region)`; an unassigned region has no row. `route_reference` = `type_id:4
/// | region_reference:8` is the routed unit. See `docs/TABLES.md`.
#[table(accessor = cold_shards, public)]
pub struct ColdShard {
    #[primary_key]
    pub route_reference: u16,
    /// Which cold family (`TYPE_BIOME_TILE` / `TYPE_BIOME_THING` / …).
    #[index(btree)]
    pub type_id: u8,
    /// The region this row routes (the high byte of `macro_position_reference`).
    #[index(btree)]
    pub region_reference: u8,
    /// The cold shard serving `(type, region)`.
    pub shard_reference: u8,
    /// That shard's endpoint (SpacetimeDB server url).
    pub url: String,
    /// Its database on that endpoint.
    pub db_name: String,
}

/// `route_reference` = `type_id:4 | region_reference:8`.
fn cold_route_reference(type_id: u8, region_reference: u8) -> u16 {
    (((type_id & 0xF) as u16) << 8) | region_reference as u16
}

/// Assign (or reassign) `(type_id, region)` to a cold shard endpoint. Upsert. The master (allocator)
/// calls this — or `rd index seed` for the dev bootstrap.
#[reducer]
pub fn set_cold_shard(
    ctx: &ReducerContext,
    type_id: u8,
    region_reference: u8,
    shard_reference: u8,
    url: String,
    db_name: String,
) -> Result<(), String> {
    let route_reference = cold_route_reference(type_id, region_reference);
    ctx.db.cold_shards().route_reference().delete(route_reference);
    ctx.db.cold_shards().insert(ColdShard {
        route_reference,
        type_id,
        region_reference,
        shard_reference,
        url,
        db_name,
    });
    Ok(())
}

/// Drop a `(type, region)` cold route.
#[reducer]
pub fn remove_cold_shard(ctx: &ReducerContext, type_id: u8, region_reference: u8) -> Result<(), String> {
    ctx.db.cold_shards().route_reference().delete(cold_route_reference(type_id, region_reference));
    Ok(())
}

// ── GC: reap dead servers + idle player pins ─────────────────────────────────

/// A server is dead once its heartbeat is this stale; reaped, its players freed.
const SERVER_TTL_MS: u64 = 60 * 1_000;
/// An idle player's pin is held this long after last activity (covers reconnect),
/// then released so the server's capacity frees.
const PLAYER_TTL_MS: u64 = 5 * 60 * 1_000;
/// Sweep cadence — below `SERVER_TTL_MS` so a dead server is noticed promptly.
const GC_INTERVAL_MS: i64 = 30 * 1_000;

/// Recurring schedule. Single row, seeded by [`init`].
// ── the simulation clock (the master's tic, per realm) ─────────────────────────
//
// The one authoritative tic for a realm. The **master** advances it (`bump_tic`); every SDK-client
// server (master, orchestrator, worker, edge) subscribes to its realm's row and reads the tic
// straight from here — the subscription push IS the fan-out. The SpacetimeDB *modules* (event/data
// shards) can't subscribe cross-database, so the master alone copies this tic into their local
// `clock` mirrors. Living on the index (the one singleton control-plane DB) means the tic survives
// any shard reset — a redeployed shard is re-stamped on the very next tic.
//
// `tic` is a `u32` absolute counter (won't wrap for ~68 years at 2 Hz); the shards take its low 16
// bits as their `master_tic` ring. Nothing compares the u32 against a shard u16 — the truncation
// happens only at the master's fan-out boundary.

#[table(accessor = master_clock, public)]
pub struct MasterClock {
    #[primary_key]
    pub realm: u8,
    pub tic: u32,
}

/// Advance a realm's tic by one. The master calls this each metronome interval; it holds no counter
/// of its own, so the durable row is the only tic that exists — a restart resumes from it. Creates
/// the row at 1 on first call.
#[reducer]
pub fn bump_tic(ctx: &ReducerContext, realm: u8) -> Result<(), String> {
    let next = ctx
        .db
        .master_clock()
        .realm()
        .find(realm)
        .map(|c| c.tic.wrapping_add(1))
        .unwrap_or(1);
    ctx.db.master_clock().realm().delete(realm);
    ctx.db.master_clock().insert(MasterClock { realm, tic: next });
    Ok(())
}

#[table(accessor = gc_schedule, scheduled(index_gc))]
pub struct GcSchedule {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub scheduled_at: ScheduleAt,
}

/// Module init — seeds the recurring GC schedule. Idempotent.
#[reducer(init)]
pub fn init(ctx: &ReducerContext) {
    if ctx.db.gc_schedule().iter().next().is_none() {
        ctx.db.gc_schedule().insert(GcSchedule {
            id: 0,
            scheduled_at: ScheduleAt::Interval(TimeDuration::from_micros(
                GC_INTERVAL_MS.saturating_mul(1_000),
            )),
        });
    }
    // Seed realm 0's clock so subscribers see a row before the master's first bump.
    if ctx.db.master_clock().realm().find(0).is_none() {
        ctx.db.master_clock().insert(MasterClock { realm: 0, tic: 0 });
    }
}

fn now_ms(ctx: &ReducerContext) -> u64 {
    (ctx.timestamp.to_micros_since_unix_epoch() / 1_000) as u64
}

/// Periodic sweep: reap servers whose heartbeat went stale (releasing their
/// pinned players so the gateway reallocates), then release idle player pins.
#[reducer]
pub fn index_gc(ctx: &ReducerContext, _row: GcSchedule) -> Result<(), String> {
    let now = now_ms(ctx);

    // Dead servers: heartbeat older than the TTL. Remove the server row and
    // release its players.
    let dead: Vec<u16> = ctx
        .db
        .servers()
        .iter()
        .filter(|s| now.saturating_sub(s.last_seen_ms) > SERVER_TTL_MS)
        .map(|s| s.server_id)
        .collect();
    for server_id in dead {
        ctx.db.servers().server_id().delete(server_id);
        release_players_of(ctx, server_id);
    }

    // Idle player pins: no activity for longer than the affinity hold. Release so
    // the server's capacity frees; a later reconnect is freshly allocated.
    let idle: Vec<u32> = ctx
        .db
        .player_servers()
        .iter()
        .filter(|p| now.saturating_sub(p.last_seen_ms) > PLAYER_TTL_MS)
        .map(|p| p.player_id)
        .collect();
    for player_id in idle {
        ctx.db.player_servers().player_id().delete(player_id);
    }

    Ok(())
}

// ── resolution helpers (the lookup chains; the gateway typically reads the
//    tables via subscription, these mirror the chains for in-module use) ───────

/// The shard id holding `region_id`, if assigned.
pub fn shard_of(ctx: &ReducerContext, region_id: u32) -> Option<u16> {
    ctx.db
        .region_shards()
        .region_id()
        .find(region_id)
        .map(|r| r.shard_id)
}

/// The full data-shard endpoint for `region_id` — region→shard→endpoint.
pub fn resolve_region(ctx: &ReducerContext, region_id: u32) -> Option<Shard> {
    let shard_id = shard_of(ctx, region_id)?;
    ctx.db.shards().shard_id().find(shard_id)
}

/// The server id a player is pinned to, if any.
pub fn server_of(ctx: &ReducerContext, player_id: u32) -> Option<u16> {
    ctx.db
        .player_servers()
        .player_id()
        .find(player_id)
        .map(|p| p.server_id)
}

/// The full server endpoint for `player_id` — player→server→endpoint.
pub fn resolve_player(ctx: &ReducerContext, player_id: u32) -> Option<Server> {
    let server_id = server_of(ctx, player_id)?;
    ctx.db.servers().server_id().find(server_id)
}
