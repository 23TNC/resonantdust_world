//! npc — the bot that drives mobile entities into the tick pipeline.
//!
//! Connects straight to the object shard (like `server_master`) and, on a timer,
//! appends `ACTION_MOVE` events; the tick pipeline (master + workers) resolves them
//! into `state`, the edge streams `state` to browsers, and the client draws each as a
//! moving sprite. Moves go direct to the shard (no edge command path); only `state`
//! travels through the edge, which keeps the browser-side sync check honest.
//!
//! Two modes, chosen by the first arg:
//!   * `demo` (default) — 5 type-tagged circles in zone (0,0) that teleport to random
//!     cells; the original cross-tab sync probe.
//!   * `wolves` — the first PAWNS: wolf packs seeded into FOREST zones near origin that
//!     wander cell-by-cell, facing their direction of travel. Bot-controlled wildlife;
//!     behaviour scales with the number of npc containers and stops if they stop.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use spacetimedb_sdk::DbContext;

use resonantdust_codec::packed::{cell, ZONE_DIM};
use resonantdust_codec::refs::{
    pack_minted_entity, ENTITY_TYPE_DEMO, ENTITY_TYPE_PAWN, SERVER_REF_NONE,
};
use resonantdust_tick::{pack_move, ACTION_MOVE};

mod bindings;
use bindings::shard::{append_event as _, seed_entity as _, DbConnection};

/// How many demo circles to drive (demo mode).
const DEMO_COUNT: u32 = 5;
/// Everything demo lives in zone (0,0) — `zone_id == 0`.
const ZONE: u32 = 0;
/// Cells per zone edge (16×16 = 256 locations).
const CELLS: u32 = 256;
/// The `from_server_id` these events are stamped with (provenance; arbitrary here).
const NPC_SERVER_ID: u16 = 9;

/// The wolf's `kind` — its `object_id` in the content thing corpus (append order in
/// `content/data/things.rd`: tree,shrub,cactus,reed,rock,flora,**wolf** = 7). The
/// client resolves this `kind` to the `pawns.animal/wolf` sprite via the thing bundle.
/// Must track that append order (a client render check catches a mismatch).
const KIND_WOLF: u16 = 7;
/// The first pawns: a small pack of wolves in zone (0,0). Kept tiny + single-zone so the
/// whole pack is on screen at the origin (phase 1). Forest-aware, multi-zone spawning is a
/// later expansion (`codec::biome::zone_is_forest`).
const WOLF_COUNT: u32 = 4;
/// Wolves sit centred in their cell (x_off:4|y_off:4, 8/16 each).
const CENTER_OFFSET: u8 = 0x88;

/// Tiny xorshift64 — the movers' motion is not part of sim determinism, so a
/// wall-clock seed is fine (no dependency needed).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn below(&mut self, n: u32) -> u32 {
        (self.next() % n as u64) as u32
    }
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn demo_key(index: u32) -> u64 {
    // Demo objects aren't shard-minted: SERVER_REF_NONE as the minting server + the index
    // as the id (self-unique for the fixed demo set).
    pack_minted_entity(ENTITY_TYPE_DEMO, index, SERVER_REF_NONE)
}

fn wolf_key(index: u32) -> u64 {
    // Wolves are pawns — same self-minted scheme, tagged PAWN so the client renders them
    // as mobile sprites (not demo circles).
    pack_minted_entity(ENTITY_TYPE_PAWN, index, SERVER_REF_NONE)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "npc=info".into()),
        )
        .init();

    let mode = std::env::args().nth(1).unwrap_or_else(|| "demo".into());
    let uri = env_or("ST_URI", "http://127.0.0.1:3000");
    let db = env_or("SHARD_DB", "resonantdust-dev-zone-0");
    tracing::info!(%uri, %db, %mode, "npc starting");

    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let ready_tx = std::sync::Mutex::new(Some(ready_tx));
    let conn = DbConnection::builder()
        .with_uri(&uri)
        .with_database_name(&db)
        .on_connect(move |_ctx, identity, _token| {
            tracing::info!(%identity, "connected");
            if let Some(tx) = ready_tx.lock().unwrap().take() {
                let _ = tx.send(());
            }
        })
        .on_connect_error(|_ctx, err| tracing::error!(%err, "connect error"))
        .on_disconnect(|_ctx, err| tracing::warn!(?err, "disconnected"))
        .build()
        .expect("build connection");
    let conn = Arc::new(conn);
    conn.run_threaded();

    // Reducer calls before the connection is live are lost; wait for on_connect.
    if tokio::time::timeout(Duration::from_secs(5), ready_rx).await.is_err() {
        tracing::error!("timed out waiting to connect");
        return;
    }

    let mut rng = Rng(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9E37_79B9_7F4A_7C15)
        | 1);

    match mode.as_str() {
        "wolves" => run_wolves(&conn, &mut rng).await,
        _ => run_demo(&conn, &mut rng).await,
    }
}

/// The original sync probe: 5 circles teleporting to random cells in zone (0,0).
async fn run_demo(conn: &Arc<DbConnection>, rng: &mut Rng) {
    let move_ms: u64 = env_or("MOVE_MS", "1000").parse().unwrap_or(1000);
    for i in 0..DEMO_COUNT {
        let loc = rng.below(CELLS) as u8;
        if let Err(err) =
            conn.reducers().seed_entity(demo_key(i), ENTITY_TYPE_DEMO as u16, ZONE, loc, 0, 0, 0, 0)
        {
            tracing::warn!(%err, i, "seed failed");
        }
    }
    tracing::info!("seeded {DEMO_COUNT} demo objects");

    let mut ticker = tokio::time::interval(Duration::from_millis(move_ms));
    loop {
        ticker.tick().await;
        for i in 0..DEMO_COUNT {
            let loc = rng.below(CELLS) as u8;
            let d = pack_move(ZONE, loc, 0, 0);
            let key = demo_key(i);
            if let Err(err) = conn.reducers().append_event(
                NPC_SERVER_ID, 0, 0, 0, 0, key, key, ACTION_MOVE, d[0], d[1],
            ) {
                tracing::warn!(%err, i, "move failed");
            }
        }
    }
}

/// The first pawns (phase 1): seed a small pack of wolves at random cells in zone (0,0),
/// then idle. `seed_entity` writes each wolf's `state` row directly (no ownership, no
/// events), so the pack simply appears on the client. Movement lands in phase 2 (the bot
/// re-seeds each wolf at a new cell — a direct state write, sidestepping the event/resolve
/// pipeline).
async fn run_wolves(conn: &Arc<DbConnection>, rng: &mut Rng) {
    // Spawn near the zone's origin corner (cells x:0..9, y:0..6) — the camera sits at world
    // (0,0) on login, so the whole pack is on screen without panning (phase 1 is about
    // *seeing* them). `ZONE_DIM` is 16; the visible slice at origin is ~the top-left third.
    for i in 0..WOLF_COUNT {
        let (x, y) = (rng.below(9) as u8, rng.below(6) as u8);
        if let Err(err) = conn.reducers().seed_entity(
            wolf_key(i), KIND_WOLF, ZONE, cell(x, y), /*rotation=*/ 0, CENTER_OFFSET, 0, 0,
        ) {
            tracing::warn!(%err, i, "wolf seed failed");
        }
    }
    tracing::info!(wolves = WOLF_COUNT, "seeded wolves in zone (0,0)");

    // Keep the connection alive (phase 1 is spawn-only; the pack just sits there).
    loop {
        tokio::time::sleep(Duration::from_secs(3600)).await;
    }
}
