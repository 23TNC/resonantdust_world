//! npc — the demo mover.
//!
//! Connects straight to the object shard (like `server_master`), seeds `DEMO_COUNT`
//! demo objects in zone (0,0) — each type-tagged in its `object_id` so the client can
//! tell them apart from real things — then every `MOVE_MS` picks a random cell for each
//! and appends an `ACTION_MOVE`. The tick pipeline resolves those into `state`; the edge
//! streams `state` to browsers that draw each as a tweening circle. Watching the same
//! objects across tabs is the sync test.
//!
//! Moves go direct to the shard (no edge command path needed); only `state` travels
//! through the edge to the browser, which keeps the browser-side sync check honest.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use spacetimedb_sdk::DbContext;

use resonantdust_codec::packed::{
    pack_object_id, pack_object_key, pack_object_reference, OBJ_TYPE_DEMO, SHARD_NONE,
};
use resonantdust_tick::{pack_move, ACTION_MOVE};

mod bindings;
use bindings::shard::{append_event as _, seed_entity as _, DbConnection};

/// How many demo circles to drive.
const DEMO_COUNT: u32 = 5;
/// Everything lives in zone (0,0) — `zone_id == 0`.
const ZONE: u32 = 0;
/// Cells per zone edge (16×16 = 256 locations).
const CELLS: u32 = 256;
/// The `from_server_id` these events are stamped with (provenance; arbitrary here).
const NPC_SERVER_ID: u16 = 9;

/// Tiny xorshift64 — the mover's motion is not part of sim determinism, so a
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
    // Demo objects aren't shard-minted: reserved SHARD_NONE + the index as the count
    // (self-unique for the fixed demo set).
    pack_object_key(pack_object_reference(
        OBJ_TYPE_DEMO,
        pack_object_id(SHARD_NONE, index),
    ))
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "npc=info".into()),
        )
        .init();

    let uri = env_or("ST_URI", "http://127.0.0.1:3000");
    let db = env_or("SHARD_DB", "resonantdust-dev-zone-0");
    let move_ms: u64 = env_or("MOVE_MS", "1000").parse().unwrap_or(1000);
    tracing::info!(%uri, %db, demo_count = DEMO_COUNT, move_ms, "npc demo mover starting");

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
    if tokio::time::timeout(Duration::from_secs(5), ready_rx)
        .await
        .is_err()
    {
        tracing::error!("timed out waiting to connect");
        return;
    }

    let mut rng = Rng(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15)
            | 1,
    );

    // Seed the demo objects at random start cells (idempotent per key).
    for i in 0..DEMO_COUNT {
        let loc = rng.below(CELLS) as u8;
        if let Err(err) =
            conn.reducers()
                .seed_entity(demo_key(i), OBJ_TYPE_DEMO as u16, ZONE, loc, 0, 0, 0, 0)
        {
            tracing::warn!(%err, i, "seed failed");
        }
    }
    tracing::info!("seeded {DEMO_COUNT} demo objects");

    // Move each to a fresh random cell every tick.
    let mut ticker = tokio::time::interval(Duration::from_millis(move_ms));
    loop {
        ticker.tick().await;
        for i in 0..DEMO_COUNT {
            let loc = rng.below(CELLS) as u8;
            let d = pack_move(ZONE, loc, 0, 0);
            let key = demo_key(i);
            if let Err(err) =
                conn.reducers()
                    .append_event(NPC_SERVER_ID, 0, key, key, ACTION_MOVE, d[0], d[1])
            {
                tracing::warn!(%err, i, "move failed");
            }
        }
        tracing::debug!("issued a round of moves");
    }
}
