//! worker — the resolver.
//!
//! It owns whole conflict-components (the orchestrator assigned them by stamping `worker_reference`).
//! For each assigned event tic it:
//!   1. **blocks** — if any target's previous (`< tic`) row is still `dirty`, it defers the whole tic
//!      and revisits next pass. This is a *correctness requirement*, not an optimisation: composing on
//!      a dirty base reads a stale value and writes a wrong-but-clean final (intent §Why 4). Never
//!      partial-write.
//!   2. **composes** the component in local scratch, from each entity's `< tic` base, applying every
//!      event's program in ascending `event_reference` order (the global total order). One worker owns
//!      the component, so this is ordinary sequential code — cross-entity transactions are just code.
//!   3. **writes** absolute finals (`data_shard.write`, one call per data shard) and **completes** each
//!      event (`event_shard.complete`, with the zones its targets ended up in).
//!
//! Writes are absolute values from immutable bases, so a dead worker's work replays identically:
//! `write` skips already-clean rows and `complete` is idempotent. The tic comes from
//! `index.master_clock`, never a shard clock.
//!
//! Scope: `PLACE` (set position) and `MOVE_TO` (arrive — multi-tile stepping + self-requeue is the
//! movement-content follow-up) and `PROMOTE_STATE`. `CREATE` is deferred: its minted id isn't a write
//! operand, so no slot is claimed for it yet (needs the orchestrator's spawn-id claim).

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use spacetimedb_sdk::{DbContext, Table as _};

use resonantdust_codec::action::{self, MOVE_TO, PLACE, PROMOTE_STATE, SET};
use resonantdust_codec::object::{pack_position_reference, position_macro, position_micro, TYPE_BIOME_THING, TYPE_BIOME_TILE};
use resonantdust_codec::refs::entity_ref_type_id;
use resonantdust_codec::status::{status_phase, EVENT_ASSIGNED};
use resonantdust_codec::tic::{tic_after, tic_before};
use resonantdust_st_bindings::{data_shard, event_shard, index, thing, tile};
use data_shard::{write as _, EntityStateLogTableAccess as _};
use event_shard::{complete as _, EventLogTableAccess as _};
use index::MasterClockTableAccess as _;
// The cold shards ride the same composition macro, so their `write` + `state_log` surfaces are the
// same shape — a cold-cell mutation composes + writes exactly like a hot one, just on its shard.
use thing::{write as _, EntityStateLogTableAccess as _};
use tile::{write as _, EntityStateLogTableAccess as _};

/// Which composing shard an entity lives on — routed by its `server_reference`'s `type_id` (the top
/// nibble). A cold cell's `SET` target carries `TYPE_BIOME_TILE`/`TYPE_BIOME_THING`; everything else
/// (a pawn's `0x30`, …) is the hot `data_shard`, unchanged.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Shard {
    Data,
    Tile,
    Thing,
}

fn shard_of(entity: u32) -> Shard {
    match entity_ref_type_id(entity) {
        TYPE_BIOME_TILE => Shard::Tile,
        TYPE_BIOME_THING => Shard::Thing,
        _ => Shard::Data,
    }
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn parse_u8(s: &str, default: u8) -> u8 {
    s.strip_prefix("0x")
        .and_then(|h| u8::from_str_radix(h, 16).ok())
        .or_else(|| s.parse().ok())
        .unwrap_or(default)
}

/// A composed entity value in scratch — the three orthogonal references of the payload.
#[derive(Clone, Copy, Default)]
struct Payload {
    definition_reference: u32,
    position_reference: u32,
    data: u8,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "worker=info".into()),
        )
        .init();

    let uri = env_or("ST_URI", "http://127.0.0.1:3000");
    let self_ref = parse_u8(&env_or("WORKER", "0x62"), 0x62);
    let realm = parse_u8(&env_or("REALM", "0"), 0);
    let tic_hz: f64 = env_or("TIC_HZ", "2").parse().unwrap_or(2.0);
    let period = Duration::from_secs_f64(1.0 / tic_hz);
    let index_db = env_or("INDEX_DB", "resonantdust-dev-index-0");
    let event_db = env_or("EVENT_DB", "resonantdust-dev-event-shard-0");
    let data_db = env_or("DATA_DB", "resonantdust-dev-data-shard-0");
    // Cold shards compose on the same machinery — the worker writes a cold-cell mutation to its shard.
    let tile_db = env_or("TILE_DB", "resonantdust-dev-tile-0");
    let thing_db = env_or("THING_DB", "resonantdust-dev-thing-0");
    tracing::info!(%uri, self_ref = format!("{self_ref:#04x}"), realm, %index_db, %event_db, %data_db, %tile_db, %thing_db, "worker starting");

    // ── index: the canonical tic ────────────────────────────────────────────────────
    let (tx_i, rx_i) = std::sync::mpsc::channel::<()>();
    let index = index::DbConnection::builder()
        .with_uri(&uri)
        .with_database_name(&index_db)
        .on_connect(|_c, id, _t| tracing::info!(%id, "index connected"))
        .on_connect_error(|_c, err| tracing::error!(%err, "index connect error"))
        .build()
        .expect("build index connection");
    index.run_threaded();
    index
        .subscription_builder()
        .on_applied(move |_| {
            let _ = tx_i.send(());
        })
        .subscribe([format!("SELECT * FROM master_clock WHERE realm = {realm}")]);

    // ── event shard: the events assigned to me, + a call surface for `complete` ──────
    let (tx_e, rx_e) = std::sync::mpsc::channel::<()>();
    let event = event_shard::DbConnection::builder()
        .with_uri(&uri)
        .with_database_name(&event_db)
        .on_connect(|_c, id, _t| tracing::info!(%id, "event_shard connected"))
        .on_connect_error(|_c, err| tracing::error!(%err, "event_shard connect error"))
        .build()
        .expect("build event_shard connection");
    event.run_threaded();
    event
        .subscription_builder()
        .on_applied(move |_| {
            let _ = tx_e.send(());
        })
        .subscribe([format!(
            "SELECT * FROM event_log WHERE worker_reference = {self_ref}"
        )]);

    // ── data shard: my slots to write + the bases I read, + a call surface for `write` ──
    let (tx_d, rx_d) = std::sync::mpsc::channel::<()>();
    let data = data_shard::DbConnection::builder()
        .with_uri(&uri)
        .with_database_name(&data_db)
        .on_connect(|_c, id, _t| tracing::info!(%id, "data_shard connected"))
        .on_connect_error(|_c, err| tracing::error!(%err, "data_shard connect error"))
        .build()
        .expect("build data_shard connection");
    data.run_threaded();
    data
        .subscription_builder()
        .on_applied(move |_| {
            let _ = tx_d.send(());
        })
        .subscribe([format!(
            "SELECT * FROM entity_state_log WHERE worker_reference = {self_ref} OR observer_reference = {self_ref}"
        )]);

    // ── cold shards: my cold slots to write + the bases I read (same overlay shape as data) ──────
    let sub_sql = format!(
        "SELECT * FROM entity_state_log WHERE worker_reference = {self_ref} OR observer_reference = {self_ref}"
    );
    let (tx_t, rx_t) = std::sync::mpsc::channel::<()>();
    let tile = tile::DbConnection::builder()
        .with_uri(&uri)
        .with_database_name(&tile_db)
        .on_connect(|_c, id, _t| tracing::info!(%id, "tile shard connected"))
        .on_connect_error(|_c, err| tracing::error!(%err, "tile shard connect error"))
        .build()
        .expect("build tile shard connection");
    tile.run_threaded();
    tile.subscription_builder().on_applied(move |_| { let _ = tx_t.send(()); }).subscribe([sub_sql.clone()]);

    let (tx_h, rx_h) = std::sync::mpsc::channel::<()>();
    let thing = thing::DbConnection::builder()
        .with_uri(&uri)
        .with_database_name(&thing_db)
        .on_connect(|_c, id, _t| tracing::info!(%id, "thing shard connected"))
        .on_connect_error(|_c, err| tracing::error!(%err, "thing shard connect error"))
        .build()
        .expect("build thing shard connection");
    thing.run_threaded();
    thing.subscription_builder().on_applied(move |_| { let _ = tx_h.send(()); }).subscribe([sub_sql.clone()]);

    for (rx, what) in [
        (&rx_i, "index"),
        (&rx_e, "event_log"),
        (&rx_d, "state_log"),
        (&rx_t, "tile state_log"),
        (&rx_h, "thing state_log"),
    ] {
        rx.recv_timeout(Duration::from_secs(5))
            .unwrap_or_else(|_| panic!("{what} subscription applied"));
    }
    tracing::info!("subscriptions applied; resolving");

    let mut ticker = tokio::time::interval(period);
    loop {
        ticker.tick().await;

        let master = index.db().master_clock().realm().find(&realm).map(|c| c.tic as u16).unwrap_or(0);

        // Gather the events still assigned to me, bucketed by their tic. Once an event is `complete`
        // its phase leaves ASSIGNED and it drops out here — no reprocessing.
        let mut per_tic: HashMap<u16, Vec<(u32, Vec<u32>)>> = HashMap::new();
        for e in event.db().event_log().iter() {
            if status_phase(e.status) != EVENT_ASSIGNED {
                continue;
            }
            per_tic.entry(e.event_tic).or_default().push((e.event_reference, e.actions.clone()));
        }
        if per_tic.is_empty() {
            continue;
        }

        // Ascending serial tic order: T's finals are T+1's base, so composing T first lets a chain
        // make progress; a not-yet-clean base just defers to the next pass.
        let mut tics: Vec<u16> = per_tic.keys().copied().collect();
        tics.sort_by(|a, b| if tic_after(*a, *b) { std::cmp::Ordering::Greater } else { std::cmp::Ordering::Less });

        for t in tics {
            let mut events = per_tic.remove(&t).unwrap();
            events.sort_by_key(|(r, _)| *r); // ascending event_reference = composition order

            // The entities this tic writes (union of every event's write targets) — each has a claimed
            // slot. Read targets ⊆ write targets under the current verbs, so blocking on these covers
            // the read block too.
            let mut targets: Vec<u32> = Vec::new();
            for (_, actions) in &events {
                if let Ok(w) = action::write_targets(actions) {
                    for e in w {
                        if !targets.contains(&e) {
                            targets.push(e);
                        }
                    }
                }
            }

            // ── BLOCK ── any target whose base (`< tic`) is still dirty ⇒ defer the whole tic. Each
            // target is read from *its own* shard (a cold cell's base lives in the tile/thing overlay).
            let mut blocked_on = None;
            for &e in &targets {
                if let Some(base) = base_row(&data, &tile, &thing, e, t) {
                    if base.dirty {
                        blocked_on = Some(e);
                        break;
                    }
                }
            }
            if let Some(e) = blocked_on {
                tracing::debug!(tic = t, entity = format!("{e:#010x}"), "base dirty — deferring tic");
                continue;
            }

            // ── COMPOSE ── scratch from each target's base, then apply every event in order.
            let mut scratch: HashMap<u32, Payload> = HashMap::new();
            for &e in &targets {
                let base = base_row(&data, &tile, &thing, e, t)
                    .map(|r| Payload {
                        definition_reference: r.definition_reference,
                        position_reference: r.position_reference,
                        data: r.data,
                    })
                    .unwrap_or_default();
                scratch.insert(e, base);
            }
            let mut promote: HashSet<u32> = HashSet::new();
            for (_, actions) in &events {
                apply(actions, &mut scratch, &mut promote);
            }

            // ── WRITE ── absolute finals, routed to each target's shard by its `server_reference`. A
            // component may span shards (an event that both moves a pawn and sets a tile); each shard
            // gets one `write` of its own targets. A cross-shard partial write just retries next pass
            // (`write` is idempotent — it skips already-clean slots).
            let mut data_w: Vec<data_shard::TargetState> = Vec::new();
            let mut tile_w: Vec<tile::TargetState> = Vec::new();
            let mut thing_w: Vec<thing::TargetState> = Vec::new();
            for &e in &targets {
                let p = scratch[&e];
                let promote = promote.contains(&e);
                match shard_of(e) {
                    Shard::Data => data_w.push(data_shard::TargetState {
                        entity_reference: e,
                        definition_reference: p.definition_reference,
                        macro_position_reference: position_macro(p.position_reference),
                        micro_position_reference: position_micro(p.position_reference),
                        data: p.data,
                        promote,
                    }),
                    Shard::Tile => tile_w.push(tile::TargetState {
                        entity_reference: e,
                        definition_reference: p.definition_reference,
                        macro_position_reference: position_macro(p.position_reference),
                        micro_position_reference: position_micro(p.position_reference),
                        data: p.data,
                        promote,
                    }),
                    Shard::Thing => thing_w.push(thing::TargetState {
                        entity_reference: e,
                        definition_reference: p.definition_reference,
                        macro_position_reference: position_macro(p.position_reference),
                        micro_position_reference: position_micro(p.position_reference),
                        data: p.data,
                        promote,
                    }),
                }
            }
            let mut write_ok = true;
            if !data_w.is_empty() {
                if let Err(err) = data.reducers().write(self_ref, t, data_w) {
                    tracing::warn!(%err, tic = t, shard = "data", "write failed — will retry next pass");
                    write_ok = false;
                }
            }
            if write_ok && !tile_w.is_empty() {
                if let Err(err) = tile.reducers().write(self_ref, t, tile_w) {
                    tracing::warn!(%err, tic = t, shard = "tile", "write failed — will retry next pass");
                    write_ok = false;
                }
            }
            if write_ok && !thing_w.is_empty() {
                if let Err(err) = thing.reducers().write(self_ref, t, thing_w) {
                    tracing::warn!(%err, tic = t, shard = "thing", "write failed — will retry next pass");
                    write_ok = false;
                }
            }
            if !write_ok {
                continue;
            }

            // ── COMPLETE ── each event, with the zones its targets ended up in.
            for (event_reference, actions) in &events {
                let mut zones: Vec<u16> = Vec::new();
                if let Ok(w) = action::write_targets(actions) {
                    for e in w {
                        let z = position_macro(scratch[&e].position_reference);
                        if !zones.contains(&z) {
                            zones.push(z);
                        }
                    }
                }
                if let Err(err) = event.reducers().complete(*event_reference, zones) {
                    tracing::warn!(%err, event = format!("{event_reference:#010x}"), "complete failed");
                }
            }
            tracing::info!(
                tic = t,
                master,
                events = events.len(),
                entities = targets.len(),
                "composed component"
            );
        }
    }
}

/// Apply one event's program to the scratch, in place. Each action writes exactly one entity's row.
fn apply(actions: &[u32], scratch: &mut HashMap<u32, Payload>, promote: &mut HashSet<u32>) {
    for inst in action::program(actions) {
        let inst = match inst {
            Ok(i) => i,
            Err(_) => return, // validated at queue; a malformed program can't be framed
        };
        match inst.action {
            PLACE => {
                // PLACE obj pos — set absolute position.
                if let [obj, pos] = inst.operands {
                    scratch.entry(*obj).or_default().position_reference = *pos;
                }
            }
            MOVE_TO => {
                // MOVE_TO obj dest — arrive in one step for now. Multi-tile stepping + self-requeue
                // is the movement-content follow-up (ACTIONS.md §Movement).
                if let [obj, dest] = inst.operands {
                    scratch.entry(*obj).or_default().position_reference = *dest;
                }
            }
            SET => {
                // SET obj def pos data — absolute full payload (the cold-cell mutation verb). The
                // composed base is irrelevant; SET overwrites. `data` is `u8` (low byte of the imm).
                if let [obj, def, pos, data] = inst.operands {
                    let p = scratch.entry(*obj).or_default();
                    p.definition_reference = *def;
                    p.position_reference = *pos;
                    p.data = *data as u8;
                }
            }
            PROMOTE_STATE => {
                // PROMOTE_STATE obj — flag the target for promotion into `state` on write.
                if let [obj] = inst.operands {
                    promote.insert(*obj);
                }
            }
            // CREATE (deferred — no claimed slot), PROMOTE_EVENT (latched at queue), NONE: no scratch effect.
            _ => {}
        }
    }
}

/// A composed base — the three payload refs + whether the slot is still dirty. Shard-agnostic (the
/// three shards' `EntityStateLog` rows are the same shape from the shared macro, but distinct Rust types).
#[derive(Clone, Copy)]
struct Base {
    definition_reference: u32,
    position_reference: u32,
    data: u8,
    dirty: bool,
}

/// The entity's base for `tic`: its most-recent visible `state_log` row strictly before `tic`, read
/// from **its own shard** (routed by `server_reference`). Visible because `claim` stamped
/// `observer_reference = self` on it (or the worker wrote it). `None` for a never-mutated entity —
/// a cold cell whose truth is still the baseline — whose base is the default (empty) payload; a `SET`
/// overwrites it absolutely, so the empty base is correct.
fn base_row(
    data: &data_shard::DbConnection,
    tile: &tile::DbConnection,
    thing: &thing::DbConnection,
    entity: u32,
    tic: u16,
) -> Option<Base> {
    macro_rules! latest {
        ($conn:expr) => {
            $conn
                .db()
                .entity_state_log()
                .iter()
                .filter(|r| r.entity_reference == entity && tic_before(r.tic, tic))
                .reduce(|a, b| if tic_after(b.tic, a.tic) { b } else { a })
                .map(|r| Base {
                    definition_reference: r.definition_reference,
                    position_reference: pack_position_reference(r.macro_position_reference, r.micro_position_reference),
                    data: r.data,
                    dirty: r.dirty,
                })
        };
    }
    match shard_of(entity) {
        Shard::Data => latest!(data),
        Shard::Tile => latest!(tile),
        Shard::Thing => latest!(thing),
    }
}
