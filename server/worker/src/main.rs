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
//! movement-content follow-up) and `PROMOTE` (a prefix). `CREATE` is deferred: its minted id isn't a write
//! operand, so no slot is claimed for it yet (needs the orchestrator's spawn-id claim).

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use spacetimedb_sdk::{DbContext, Table as _};

use resonantdust_codec::action::{
    self, Route, BUILD_WALL, CREATE, INIT_ZONE, MOVE_STEP, MOVE_TO, PLACE, PROMOTE, SET,
};
use resonantdust_codec::object::{
    cold_row_layer_id, cold_row_macro_position, cold_row_subtype, data_rotation, kind_pos_ref_data,
    kind_pos_ref_kind_reference, kind_pos_ref_tile, pack_cold_row_reference, pack_kind_reference,
    pack_pawn_data, pack_position_reference, pawn_trip_serial, position_macro, position_micro,
    position_to_tile, tile_to_position, TYPE_BIOME_THING, TYPE_BIOME_TILE, TYPE_PAWN,
};
use resonantdust_codec::speed;
use resonantdust_codec::refs::entity_ref_type_id;
use resonantdust_codec::status::{status_phase, EVENT_ASSIGNED};
use resonantdust_codec::tic::{tic_add, tic_after, tic_before};
use resonantdust_st_bindings::{data_shard, event_shard, index, pawn, thing, tile};
use resonantdust_uplink::acquire;
use data_shard::{write as _, EntityStateLogTableAccess as _};
use pawn::{spawn as _, write as _, EntityStateLogTableAccess as _};
use event_shard::{complete as _, queue_at as _, EventLogTableAccess as _};
use index::MasterClockTableAccess as _;
// P4: the worker also composes cold rows — the **baseline** (`INIT_ZONE` → `write`) and the **overlay**
// (`SET` → `write_overlay`, reading the `overlay_log` base).
use tile::{write as _, write_overlay as _, OverlayLogTableAccess as _};
use thing::{write as _, write_overlay as _, OverlayLogTableAccess as _};
// P3: the cold shards (tile/thing) are now `cold_row_reference`-addressed (dense/sparse `entity_state`
// + a sparse `overlay`) — a cold cell no longer has a per-entity `entity_state_log` slot the worker
// composes. Cold writes go through the shards' direct `seed`/`set_*`/`fold` reducers for now; the
// worker's **cold-row** composition (whole-`Vec` scratch → `write`/`write_overlay` + `PROMOTE`) is P4
// (`docs/work/shard-tables/`). The worker still connects to the cold shards below (their tic clocks +
// future cold-row slots) but neither reads a cold base nor writes a cold target this phase.

/// Which composing shard an entity lives on — routed by its `server_reference`'s `type_id` (the top
/// nibble). A cold cell's `SET` target carries `TYPE_BIOME_TILE`/`TYPE_BIOME_THING`; a pawn's `0x30`
/// is the `pawn` shard (first-pawns); anything else is the hot `data_shard` catch-all.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Shard {
    Data,
    Pawn,
    Tile,
    Thing,
}

/// Per-kind tics-per-tile, `speeds[object_id - 1]`, pre-resolved through
/// [`speed::resolve`] at load (so no zero entries). Reads `data/*.rd`, `visual/*.rd`,
/// `material/*.rd` sorted — the edge's `load_disk` order — so `object_id`s agree.
fn load_speeds(root: &str) -> Result<Vec<u16>, String> {
    let mut sources: Vec<(String, String)> = Vec::new();
    for facet in ["data", "visual", "material"] {
        let dir = std::path::Path::new(root).join(facet);
        if !dir.is_dir() {
            continue;
        }
        let mut entries: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
            .map_err(|e| format!("{}: {e}", dir.display()))?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "rd"))
            .collect();
        entries.sort();
        for path in entries {
            let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let file = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
            sources.push((format!("{facet}/{file}"), text));
        }
    }
    if sources.is_empty() {
        return Err(format!("no .rd sources under {root}"));
    }
    let bundle = resonantdust_dsl::loader::load(&sources)
        .map_err(|errs| format!("corpus load: {} error(s), first: {:?}", errs.len(), errs.first()))?;
    Ok((1..=bundle.thing_names().len() as u16)
        .map(|id| speed::resolve(bundle.thing_speed(id)))
        .collect())
}

/// A pawn's hop cost by its `definition_reference` (= content `object_id` for CREATE-minted
/// pawns; `0` for legacy PLACE-only rows → the default).
fn tics_for(speeds: &[u16], def: u32) -> u16 {
    def.checked_sub(1)
        .and_then(|i| speeds.get(i as usize).copied())
        .unwrap_or(speed::DEFAULT_TICS_PER_TILE)
}

fn shard_of(entity: u32) -> Shard {
    match entity_ref_type_id(entity) {
        TYPE_BIOME_TILE => Shard::Tile,
        TYPE_BIOME_THING => Shard::Thing,
        TYPE_PAWN => Shard::Pawn,
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
    let tic_hz: f64 = env_or("TIC_HZ", &resonantdust_codec::tic::TIC_HZ.to_string()).parse().unwrap_or(resonantdust_codec::tic::TIC_HZ as f64);
    let period = Duration::from_secs_f64(1.0 / tic_hz);
    let index_db = env_or("INDEX_DB", "resonantdust-dev-index-0");
    let event_db = env_or("EVENT_DB", "resonantdust-dev-event-shard-0");
    let data_db = env_or("DATA_DB", "resonantdust-dev-data-shard-0");
    let pawn_db = env_or("PAWN_DB", "resonantdust-dev-pawn-0");
    // Cold shards compose on the same machinery — the worker writes a cold-cell mutation to its shard.
    let tile_db = env_or("TILE_DB", "resonantdust-dev-tile-0");
    let thing_db = env_or("THING_DB", "resonantdust-dev-thing-0");
    tracing::info!(%uri, self_ref = format!("{self_ref:#04x}"), realm, %index_db, %event_db, %data_db, %pawn_db, %tile_db, %thing_db, "worker starting");

    // Speed is CONTENT (pawn-movement F1/F5): resolve each kind's authored tics-per-tile from
    // the disk corpus once at startup (the sim container mounts the repo; def-ids agree with
    // the edge because the file order mirrors its `load_disk`). A missing/broken corpus is a
    // MISCONFIG, not an outage — running anyway would step every pawn at the default while
    // clients speculate authored speeds, so exit for the restart policy instead of drifting.
    let content_dir = env_or("CONTENT_DIR", "/workspace/content");
    let speeds = match load_speeds(&content_dir) {
        Ok(s) => {
            tracing::info!(%content_dir, kinds = s.len(), "corpus speeds loaded");
            s
        }
        Err(err) => {
            tracing::error!(%err, %content_dir, "content corpus load failed; exiting for the restart policy to retry");
            std::process::exit(1);
        }
    };

    // ── uplinks (sim-self-heal P3): every upstream is lazy + self-healing. The subscribed
    // ones gate `get()` on their cache applying, so the pass never reads an un-applied mirror;
    // startup and mid-run recovery are the same code path. ────────────────────────────────────
    let index_up = resonantdust_uplink::subbed_uplink!(index, "index", uri, index_db,
        vec![format!("SELECT * FROM master_clock WHERE realm = {realm}")]);
    let event_up = resonantdust_uplink::subbed_uplink!(event_shard, "event_shard", uri, event_db,
        vec![format!("SELECT * FROM event_log WHERE worker_reference = {self_ref}")]);
    let hot_sql = |_: ()| format!("SELECT * FROM entity_state_log WHERE worker_reference = {self_ref} OR observer_reference = {self_ref}");
    let data_up = resonantdust_uplink::subbed_uplink!(data_shard, "data_shard", uri, data_db, vec![hot_sql(())]);
    let pawn_up = resonantdust_uplink::subbed_uplink!(pawn, "pawn", uri, pawn_db, vec![hot_sql(())]);
    let cold_sql: Vec<String> = ["entity_state_log", "overlay_log"]
        .iter()
        .map(|t| format!("SELECT * FROM {t} WHERE worker_reference = {self_ref} OR observer_reference = {self_ref}"))
        .collect();
    let tile_up = resonantdust_uplink::subbed_uplink!(tile, "tile", uri, tile_db, cold_sql.clone());
    let thing_up = resonantdust_uplink::subbed_uplink!(thing, "thing", uri, thing_db, cold_sql.clone());

    for (name, ok) in [
        ("index", index_up.get().await.is_ok()),
        ("event_shard", event_up.get().await.is_ok()),
        ("data_shard", data_up.get().await.is_ok()),
        ("pawn", pawn_up.get().await.is_ok()),
        ("tile", tile_up.get().await.is_ok()),
        ("thing", thing_up.get().await.is_ok()),
    ] {
        if !ok {
            tracing::warn!(%name, "not reachable at startup; will keep retrying");
        }
    }
    tracing::info!("resolving (uplinks lazy — a pass needing a dead upstream defers)");
    let (mut up_i, mut up_e, mut up_d, mut up_p, mut up_t, mut up_h) = (true, true, true, true, true, true);

    let mut ticker = tokio::time::interval(period);
    loop {
        ticker.tick().await;

        // Acquire everything a pass may touch; a dead upstream DEFERS the pass (events stay
        // ASSIGNED, writes are absolute + replay-safe — re-composition after a heal is correct).
        let Some(index) = acquire(&index_up, &mut up_i, "index").await else { continue };
        let Some(event) = acquire(&event_up, &mut up_e, "event_shard").await else { continue };
        let Some(data) = acquire(&data_up, &mut up_d, "data_shard").await else { continue };
        let Some(pawn) = acquire(&pawn_up, &mut up_p, "pawn").await else { continue };
        let Some(tile) = acquire(&tile_up, &mut up_t, "tile").await else { continue };
        let Some(thing) = acquire(&thing_up, &mut up_h, "thing").await else { continue };

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

            // The entities this tic writes (union of every event's write targets) + how each routes
            // (F11): hot (`data_shard`, composed via `apply`) vs cold **overlay** (`SET`, composed as a
            // whole row below). Read targets ⊆ write targets under the current verbs, so blocking on the
            // write targets covers the read block too.
            let mut targets: Vec<u32> = Vec::new();
            let mut route_of: HashMap<u32, Route> = HashMap::new();
            for (_, actions) in &events {
                if let Ok(w) = action::write_targets(actions) {
                    for e in w {
                        if !targets.contains(&e) {
                            targets.push(e);
                        }
                    }
                }
                if let Ok(rs) = action::target_routes(actions) {
                    for (tgt, r) in rs {
                        route_of.entry(tgt).or_insert(r);
                    }
                }
            }
            let hot_targets: Vec<u32> = targets
                .iter()
                .copied()
                .filter(|e| !matches!(route_of.get(e), Some(Route::ColdOverlay { .. }) | Some(Route::ColdBaseline { .. })))
                .collect();
            // The cold overlay (`SET`) cells this tic writes, grouped per cold_row (+ its promote bit).
            let cold_sets = collect_cold_overlay(&events);

            // A cold_row's latest `overlay_log` strictly before `tic`, on `$conn` (tile/thing `OverlayLog`
            // are distinct types, so this inlines per call rather than routing through a shared fn).
            macro_rules! overlay_latest {
                ($conn:expr, $row:expr) => {
                    $conn
                        .db()
                        .overlay_log()
                        .iter()
                        .filter(|r| r.cold_row_reference == $row && tic_before(r.tic, t))
                        .reduce(|a, b| if tic_after(b.tic, a.tic) { b } else { a })
                };
            }

            // ── BLOCK ── any target whose base (`< tic`) is still dirty ⇒ defer the whole tic. Hot bases
            // live in `data_shard.entity_state_log`; a cold overlay base in the shard's `overlay_log`.
            let mut blocked_on = None;
            for &e in &hot_targets {
                if base_row(&data, &pawn, e, t).map(|b| b.dirty).unwrap_or(false) {
                    blocked_on = Some(e);
                    break;
                }
            }
            if blocked_on.is_none() {
                for cold_row in cold_sets.keys() {
                    let dirty = match route_of.get(cold_row) {
                        Some(Route::ColdOverlay { type_id }) if *type_id == TYPE_BIOME_TILE => {
                            overlay_latest!(tile, *cold_row).map(|r| r.dirty).unwrap_or(false)
                        }
                        Some(Route::ColdOverlay { type_id }) if *type_id == TYPE_BIOME_THING => {
                            overlay_latest!(thing, *cold_row).map(|r| r.dirty).unwrap_or(false)
                        }
                        _ => false,
                    };
                    if dirty {
                        blocked_on = Some(*cold_row);
                        break;
                    }
                }
            }
            if let Some(e) = blocked_on {
                tracing::debug!(tic = t, target = format!("{e:#010x}"), "base dirty — deferring tic");
                continue;
            }

            // ── COMPOSE (hot) ── scratch from each hot target's base, then apply every event in order.
            // `apply` skips `SET` (a cold verb) — the overlay is composed below.
            let mut scratch: HashMap<u32, Payload> = HashMap::new();
            for &e in &hot_targets {
                let base = base_row(&data, &pawn, e, t)
                    .map(|r| Payload {
                        definition_reference: r.definition_reference,
                        position_reference: r.position_reference,
                        data: r.data,
                    })
                    .unwrap_or_default();
                scratch.insert(e, base);
            }
            let mut promote: HashSet<u32> = HashSet::new();
            for (event_reference, actions) in &events {
                apply(actions, &mut scratch, &mut promote, *event_reference);
            }

            // ── WRITE (hot) ── absolute finals, each to its OWN shard (pawn vs the data catch-all).
            let data_w: Vec<data_shard::TargetState> = scratch
                .iter()
                .filter(|(&e, _)| shard_of(e) == Shard::Data)
                .map(|(&e, p)| data_shard::TargetState {
                    entity_reference: e,
                    definition_reference: p.definition_reference,
                    macro_position_reference: position_macro(p.position_reference),
                    micro_position_reference: position_micro(p.position_reference),
                    data: p.data,
                    promote: promote.contains(&e),
                })
                .collect();
            if !data_w.is_empty() {
                if let Err(err) = data.reducers().write(self_ref, t, data_w) {
                    tracing::warn!(%err, tic = t, shard = "data", "write failed — will retry next pass");
                    continue;
                }
            }
            let pawn_w: Vec<pawn::TargetState> = scratch
                .iter()
                .filter(|(&e, _)| shard_of(e) == Shard::Pawn)
                .map(|(&e, p)| pawn::TargetState {
                    entity_reference: e,
                    definition_reference: p.definition_reference,
                    macro_position_reference: position_macro(p.position_reference),
                    micro_position_reference: position_micro(p.position_reference),
                    data: p.data,
                    promote: promote.contains(&e),
                })
                .collect();
            if !pawn_w.is_empty() {
                if let Err(err) = pawn.reducers().write(self_ref, t, pawn_w) {
                    tracing::warn!(%err, tic = t, shard = "pawn", "write failed — will retry next pass");
                    continue;
                }
            }

            // ── COMPOSE + WRITE (cold overlay) ── each cold_row's whole overlay row = its current base
            // (`overlay_latest`) with this tic's `SET` cells merged in by `tile_reference` (a `kind==0`
            // cell stays as a clear-marker so the edge relays a removal), then `write_overlay` per shard.
            // (`init_zone`/`PACK` — the `ColdBaseline` tier — are a later P4 increment.)
            let mut tile_ov: Vec<tile::OverlayTarget> = Vec::new();
            let mut thing_ov: Vec<thing::OverlayTarget> = Vec::new();
            for (&cold_row, (cells, promoted)) in &cold_sets {
                match route_of.get(&cold_row) {
                    Some(Route::ColdOverlay { type_id }) if *type_id == TYPE_BIOME_TILE => {
                        let mut items = overlay_latest!(tile, cold_row).map(|r| r.items).unwrap_or_default();
                        for c in cells {
                            items.retain(|it| it.tile_reference != c.tile_reference);
                            items.push(tile::OverlayItem { tile_reference: c.tile_reference, kind_reference: c.kind_reference });
                        }
                        tile_ov.push(tile::OverlayTarget {
                            cold_row_reference: cold_row,
                            macro_position_reference: cold_row_macro_position(cold_row),
                            subtype_id: cold_row_subtype(cold_row),
                            layer_id: cold_row_layer_id(cold_row),
                            items,
                            promote: *promoted,
                        });
                    }
                    Some(Route::ColdOverlay { type_id }) if *type_id == TYPE_BIOME_THING => {
                        let mut items = overlay_latest!(thing, cold_row).map(|r| r.items).unwrap_or_default();
                        for c in cells {
                            items.retain(|it| it.tile_reference != c.tile_reference);
                            items.push(thing::OverlayItem { tile_reference: c.tile_reference, kind_reference: c.kind_reference, data: c.data });
                        }
                        thing_ov.push(thing::OverlayTarget {
                            cold_row_reference: cold_row,
                            macro_position_reference: cold_row_macro_position(cold_row),
                            subtype_id: cold_row_subtype(cold_row),
                            layer_id: cold_row_layer_id(cold_row),
                            items,
                            promote: *promoted,
                        });
                    }
                    _ => {} // ColdBaseline (init_zone/PACK) — a later increment.
                }
            }
            if !tile_ov.is_empty() {
                if let Err(err) = tile.reducers().write_overlay(self_ref, t, tile_ov) {
                    tracing::warn!(%err, tic = t, shard = "tile", "write_overlay failed — will retry next pass");
                    continue;
                }
            }
            if !thing_ov.is_empty() {
                if let Err(err) = thing.reducers().write_overlay(self_ref, t, thing_ov) {
                    tracing::warn!(%err, tic = t, shard = "thing", "write_overlay failed — will retry next pass");
                    continue;
                }
            }

            // ── COMPOSE + WRITE (cold baseline) ── each `INIT_ZONE` builds a whole baseline row from its
            // event payload (absolute — no base read), interpreted per shard, then `write` + `PROMOTE`
            // (the event-driven `seed`, F12). `PACK` (fold overlay → baseline) is a later increment.
            let cold_inits = collect_cold_baseline(&events);
            let mut tile_base: Vec<tile::TargetState> = Vec::new();
            let mut thing_base: Vec<thing::TargetState> = Vec::new();
            for (&cold_row, (items, promoted)) in &cold_inits {
                match route_of.get(&cold_row) {
                    Some(Route::ColdBaseline { type_id }) if *type_id == TYPE_BIOME_TILE => {
                        // Dense: each item word is a `kind_reference`, indexed by position.
                        let dense = items.iter().map(|&k| tile::DenseItem { kind_reference: k as u16 }).collect();
                        tile_base.push(tile::TargetState {
                            cold_row_reference: cold_row,
                            macro_position_reference: cold_row_macro_position(cold_row),
                            subtype_id: cold_row_subtype(cold_row),
                            layer_id: cold_row_layer_id(cold_row),
                            items: dense,
                            promote: *promoted,
                        });
                    }
                    Some(Route::ColdBaseline { type_id }) if *type_id == TYPE_BIOME_THING => {
                        // Sparse: each item word is a `kind_pos_reference` (kind:16 | tile:8 | data:8).
                        let sparse = items
                            .iter()
                            .map(|&e| thing::DenseItem {
                                tile_reference: kind_pos_ref_tile(e),
                                kind_reference: kind_pos_ref_kind_reference(e),
                                data: kind_pos_ref_data(e),
                            })
                            .collect();
                        thing_base.push(thing::TargetState {
                            cold_row_reference: cold_row,
                            macro_position_reference: cold_row_macro_position(cold_row),
                            subtype_id: cold_row_subtype(cold_row),
                            layer_id: cold_row_layer_id(cold_row),
                            items: sparse,
                            promote: *promoted,
                        });
                    }
                    _ => {}
                }
            }
            if !tile_base.is_empty() {
                if let Err(err) = tile.reducers().write(self_ref, t, tile_base) {
                    tracing::warn!(%err, tic = t, shard = "tile", "baseline write failed — will retry next pass");
                    continue;
                }
            }
            if !thing_base.is_empty() {
                if let Err(err) = thing.reducers().write(self_ref, t, thing_base) {
                    tracing::warn!(%err, tic = t, shard = "thing", "baseline write failed — will retry next pass");
                    continue;
                }
            }

            // ── SPAWN (CREATE) ── each `CREATE` mints server-side via the pawn shard's idempotent
            // `spawn` (`ACTIONS.md` §CREATE): `(event, index)` keys the replay ledger, so a re-pass
            // re-calls harmlessly. The minted id is not an operand — nothing claimed it and nothing
            // else this tic can reference it. A failed call defers the whole tic (like a write).
            let mut create_zones: HashMap<u32, Vec<u16>> = HashMap::new();
            let mut spawn_failed = false;
            for (event_reference, actions) in &events {
                let mut pending_promote = false;
                let mut index: u16 = 0;
                for inst in action::program(actions) {
                    let Ok(inst) = inst else { break };
                    match inst.action {
                        PROMOTE => {
                            pending_promote = true;
                            continue;
                        }
                        CREATE => {
                            if let [def, pos] = inst.operands {
                                if let Err(err) = pawn
                                    .reducers()
                                    .spawn(self_ref, t, *event_reference, index, *def, *pos, pending_promote)
                                {
                                    tracing::warn!(%err, tic = t, "spawn failed — will retry next pass");
                                    spawn_failed = true;
                                }
                                create_zones.entry(*event_reference).or_default().push(position_macro(*pos));
                                index += 1;
                            }
                        }
                        _ => {}
                    }
                    pending_promote = false;
                }
            }
            if spawn_failed {
                continue;
            }

            // ── CONTINUE (MOVE_TO seed / MOVE_STEP hop) ── each un-arrived move queues its next
            // `MOVE_STEP` hop at `tic + tics_per_tile` (ACTIONS.md §Movement): BARE while distance
            // > 1, `PROMOTE`-prefixed when the next hop lands (the final-tile state anchor). The
            // hop carries the chain's trip-serial — a seed chains its OWN (`event_reference` low
            // bits, just stamped by apply), a hop chains its operand — and a chain whose serial no
            // longer owns the pawn is superseded: it queues NOTHING and dies here. Best-effort BY
            // CHOICE: a failed queue kills the chain (the driver re-issues on timeout — SAFE now,
            // supersession makes the re-issue cancel any survivor); deferring the tic would
            // re-queue on the re-pass and DUPLICATE the chain, which multiplies. Never defer here.
            for (event_reference, actions) in &events {
                for inst in action::program(actions) {
                    let Ok(inst) = inst else { break };
                    let (obj, dest, serial) = match (inst.action, inst.operands) {
                        (MOVE_TO, [obj, dest]) => (*obj, *dest, event_reference & 0x3F),
                        (MOVE_STEP, [obj, dest, serial]) => (*obj, *dest, *serial & 0x3F),
                        _ => continue,
                    };
                    let Some(p) = scratch.get(&obj) else { continue };
                    if u32::from(pawn_trip_serial(p.data)) != serial {
                        continue; // superseded — the chain ends (apply logged the death)
                    }
                    let (cx, cy) = position_to_tile(p.position_reference);
                    let (tx, ty) = position_to_tile(dest);
                    let d = (tx - cx).abs().max((ty - cy).abs());
                    if d == 0 {
                        continue; // arrived — the chain ends
                    }
                    let next_tic = tic_add(t, tics_for(&speeds, p.definition_reference));
                    let program = if d == 1 {
                        vec![PROMOTE, MOVE_STEP, obj, dest, serial]
                    } else {
                        vec![MOVE_STEP, obj, dest, serial]
                    };
                    if let Err(err) = event.reducers().queue_at(program, next_tic) {
                        tracing::warn!(%err, tic = t, obj = format!("{obj:#010x}"), "continuation queue failed — chain ends");
                    }
                }
            }

            // ── BUILD (BUILD_WALL start end object) ── expand the rect's PERIMETER and queue one
            // `PROMOTE SET` per tile at the next tic (ACTIONS.md §Building): each SET routes to
            // ITS OWN zone through the normal queue, so a multi-zone rect is safe by construction.
            // IMMEDIATE building is THIS pass's policy, not the verb's contract — the documented
            // future queues blueprint-entity creates here instead. Best-effort like the movement
            // continuations: a failed queue drops that tile's wall (the client can re-issue).
            for (_event_reference, actions) in &events {
                for inst in action::program(actions) {
                    let Ok(inst) = inst else { break };
                    let (start, end, object) = match (inst.action, inst.operands) {
                        (BUILD_WALL, [s, e, o]) => (*s, *e, *o),
                        _ => continue,
                    };
                    let (sx, sy) = position_to_tile(start);
                    let (ex, ey) = position_to_tile(end);
                    let (x0, x1) = (sx.min(ex), sx.max(ex));
                    let (y0, y1) = (sy.min(ey), sy.max(ey));
                    let next_tic = tic_add(t, 1);
                    let kind = u32::from(pack_kind_reference(object as u16, 0));
                    let mut queued = 0u32;
                    for x in x0..=x1 {
                        for y in y0..=y1 {
                            if x != x0 && x != x1 && y != y0 && y != y1 {
                                continue; // interior — walls stand on the perimeter only
                            }
                            let pos = tile_to_position(x, y);
                            let cold_row = pack_cold_row_reference(position_macro(pos), 0, 0);
                            let tile_reference = u32::from((position_micro(pos) >> 8) as u8);
                            let program = vec![
                                PROMOTE, SET, cold_row, u32::from(TYPE_BIOME_TILE), tile_reference, kind, 0,
                            ];
                            if let Err(err) = event.reducers().queue_at(program, next_tic) {
                                tracing::warn!(%err, tic = t, x, y, "build_wall SET queue failed — tile dropped");
                            } else {
                                queued += 1;
                            }
                        }
                    }
                    tracing::info!(tic = t, x0, y0, x1, y1, object, queued, "build_wall expanded");
                }
            }

            // ── COMPLETE ── each event, with the zones its targets ended up in (a hot target's zone from
            // its composed position; a cold_row's from its address; a CREATE's from its spawn position).
            for (event_reference, actions) in &events {
                let mut zones: Vec<u16> = Vec::new();
                if let Ok(w) = action::write_targets(actions) {
                    for e in w {
                        let z = scratch
                            .get(&e)
                            .map(|p| position_macro(p.position_reference))
                            .unwrap_or_else(|| cold_row_macro_position(e));
                        if !zones.contains(&z) {
                            zones.push(z);
                        }
                    }
                }
                for z in create_zones.get(event_reference).into_iter().flatten() {
                    if !zones.contains(z) {
                        zones.push(*z);
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
                hot = scratch.len(),
                cold_overlay = cold_sets.len(),
                cold_baseline = cold_inits.len(),
                "composed component"
            );
        }
    }
}

/// Apply one event's program to the scratch, in place. `PROMOTE` is a **prefix**: it flags the *next*
/// action, whose write targets then join `promote` (they project to the client-visible table on write).
/// `event_reference` mints the trip-serial a `MOVE_TO` seed stamps (chain identity — ACTIONS.md
/// §Movement; unique even for two same-tic intents, where a tic-derived serial would collide).
fn apply(actions: &[u32], scratch: &mut HashMap<u32, Payload>, promote: &mut HashSet<u32>, event_reference: u32) {
    let mut pending_promote = false; // set by a `PROMOTE` prefix; consumed by the next action
    for inst in action::program(actions) {
        let inst = match inst {
            Ok(i) => i,
            Err(_) => return, // validated at queue; a malformed program can't be framed
        };
        match inst.action {
            PROMOTE => {
                // Prefix — flag the next action's write targets; don't clear the flag here.
                pending_promote = true;
                continue;
            }
            PLACE => {
                // PLACE obj pos — set absolute position.
                if let [obj, pos] = inst.operands {
                    scratch.entry(*obj).or_default().position_reference = *pos;
                    if pending_promote {
                        promote.insert(*obj);
                    }
                }
            }
            MOVE_TO => {
                // MOVE_TO obj dest — ALWAYS the seed (ACTIONS.md §Movement): stamp the
                // trip-serial (this event's low 6 bits — the chain's identity, killing any
                // OLDER chain at its next hop), turn facing toward the path (e/w winning
                // diagonals — side profiles read best), step NOTHING. The promoted write is
                // the anchor that aligns every client to the server BEFORE speculation walks
                // (pawn-movement I6: a stepping seed fanned start+1 — a snap opening every
                // trip). The worker chains `MOVE_STEP` hops from the CONTINUE pass.
                if let [obj, dest] = inst.operands {
                    let p = scratch.entry(*obj).or_default();
                    let (cx, cy) = position_to_tile(p.position_reference);
                    let (tx, ty) = position_to_tile(*dest);
                    let (sx, sy) = ((tx - cx).signum(), (ty - cy).signum());
                    let facing: u8 = if sx > 0 { 1 } else if sx < 0 { 3 } else if sy > 0 { 0 }
                        else if sy < 0 { 2 } else { data_rotation(p.data) };
                    p.data = pack_pawn_data(facing, (event_reference & 0x3F) as u8);
                    if pending_promote {
                        promote.insert(*obj);
                    }
                }
            }
            MOVE_STEP => {
                // MOVE_STEP obj dest serial — one chain hop: step ONE tile toward dest
                // (greedy straight line — the pathfinding seam) ONLY while `serial` still
                // matches the pawn's stamp. A mismatch means a newer intent superseded this
                // chain — die silently (the CONTINUE pass re-checks the same predicate, so
                // no re-queue happens either).
                if let [obj, dest, serial] = inst.operands {
                    let p = scratch.entry(*obj).or_default();
                    if u32::from(pawn_trip_serial(p.data)) != (serial & 0x3F) {
                        tracing::debug!(
                            obj = format!("{obj:#010x}"), serial,
                            stamped = pawn_trip_serial(p.data),
                            "chain superseded — hop dies"
                        );
                    } else {
                        let (cx, cy) = position_to_tile(p.position_reference);
                        let (tx, ty) = position_to_tile(*dest);
                        let (sx, sy) = ((tx - cx).signum(), (ty - cy).signum());
                        if sx != 0 || sy != 0 {
                            p.position_reference = tile_to_position(cx + sx, cy + sy);
                            let facing: u8 = if sx > 0 { 1 } else if sx < 0 { 3 } else if sy > 0 { 0 } else { 2 };
                            p.data = pack_pawn_data(facing, pawn_trip_serial(p.data));
                        }
                    }
                    if pending_promote {
                        promote.insert(*obj);
                    }
                }
            }
            // SET is a **cold overlay** verb (targets a cold_row, not a hot entity) — composed as a
            // whole overlay row in the tic loop (`collect_cold_overlay` + `write_overlay`), not here.
            // CREATE mints via the pawn shard's `spawn` in the tic loop (no scratch — the minted id
            // is not an operand). PROMOTE_EVENT (latched at queue), NONE: no scratch effect.
            _ => {}
        }
        pending_promote = false; // any non-PROMOTE action consumes the prefix
    }
}

/// One overlay cell a `SET` writes this tic — the payload merged into its cold_row's `overlay` row.
struct ColdCell {
    tile_reference: u8,
    kind_reference: u16,
    data: u8,
}

/// Group this tic's `SET` cells per cold_row target (event + program order), with the row's promote
/// bit (a `PROMOTE` prefix on any of its `SET`s promotes the whole composed overlay row). Only `SET`
/// writes the overlay; hot verbs are composed via [`apply`], cold-baseline verbs in a later increment.
fn collect_cold_overlay(events: &[(u32, Vec<u32>)]) -> HashMap<u32, (Vec<ColdCell>, bool)> {
    let mut out: HashMap<u32, (Vec<ColdCell>, bool)> = HashMap::new();
    for (_, actions) in events {
        let mut pending_promote = false; // a PROMOTE prefix; consumed by the next action
        for inst in action::program(actions) {
            let inst = match inst {
                Ok(i) => i,
                Err(_) => break, // validated at queue; a malformed program can't be framed
            };
            match inst.action {
                PROMOTE => {
                    pending_promote = true;
                    continue;
                }
                // SET cold_row type_id tile_reference kind_reference data.
                SET => {
                    if let [cold_row, _type_id, tile_reference, kind_reference, data] = inst.operands {
                        let entry = out.entry(*cold_row).or_insert_with(|| (Vec::new(), false));
                        entry.0.push(ColdCell {
                            tile_reference: *tile_reference as u8,
                            kind_reference: *kind_reference as u16,
                            data: *data as u8,
                        });
                        if pending_promote {
                            entry.1 = true;
                        }
                    }
                }
                _ => {}
            }
            pending_promote = false;
        }
    }
    out
}

/// Group this tic's `INIT_ZONE` baseline payloads per cold_row target (+ promote bit). The raw `item`
/// words are the row's whole payload — interpreted per shard at write (tile: dense `kind_reference` by
/// index; thing: `kind_pos_reference`). `INIT_ZONE` is absolute (no base read), like a whole-row `SET`.
fn collect_cold_baseline(events: &[(u32, Vec<u32>)]) -> HashMap<u32, (Vec<u32>, bool)> {
    let mut out: HashMap<u32, (Vec<u32>, bool)> = HashMap::new();
    for (_, actions) in events {
        let mut pending_promote = false;
        for inst in action::program(actions) {
            let inst = match inst {
                Ok(i) => i,
                Err(_) => break,
            };
            match inst.action {
                PROMOTE => {
                    pending_promote = true;
                    continue;
                }
                // INIT_ZONE cold_row type_id count item×count — the items are operands[3..].
                INIT_ZONE => {
                    if let [cold_row, _type_id, _count, items @ ..] = inst.operands {
                        // Last write wins if a row is re-inited in one tic (absolute payload).
                        out.insert(*cold_row, (items.to_vec(), pending_promote));
                    }
                }
                _ => {}
            }
            pending_promote = false;
        }
    }
    out
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
/// overwrites it absolutely, so the empty base is correct. The two hot shards' `EntityStateLog` rows
/// are the same macro shape but distinct Rust types, hence the per-arm body.
fn base_row(data: &data_shard::DbConnection, pawn: &pawn::DbConnection, entity: u32, tic: u16) -> Option<Base> {
    macro_rules! latest_base {
        ($conn:expr) => {{
            let newest = $conn
                .db()
                .entity_state_log()
                .iter()
                .filter(|r| r.entity_reference == entity && tic_before(r.tic, tic))
                .reduce(|a, b| if tic_after(b.tic, a.tic) { b } else { a });
            // ABANDON rule (movement-hardening I3): a dirty slot older than ABANDON_TICS will
            // never be written (its worker died or its event vanished — live latency is 2–3
            // tics), so blocking on it wedges the entity FOREVER. Base on the latest CLEAN
            // row past it instead; the clean row the next compose writes lets `gc` reap the
            // orphan. A dirty slot YOUNGER than the window still blocks (legitimate pending).
            let newest = match newest {
                Some(r) if r.dirty && tic_before(r.tic, tic.wrapping_sub(resonantdust_codec::tic::ABANDON_TICS)) => {
                    tracing::debug!(entity = format!("{entity:#010x}"), slot_tic = r.tic, tic, "dirty base abandoned — using latest clean row");
                    $conn
                        .db()
                        .entity_state_log()
                        .iter()
                        .filter(|r| r.entity_reference == entity && tic_before(r.tic, tic) && !r.dirty)
                        .reduce(|a, b| if tic_after(b.tic, a.tic) { b } else { a })
                }
                other => other,
            };
            newest.map(|r| Base {
                definition_reference: r.definition_reference,
                position_reference: pack_position_reference(r.macro_position_reference, r.micro_position_reference),
                data: r.data,
                dirty: r.dirty,
            })
        }};
    }
    match shard_of(entity) {
        Shard::Data => latest_base!(data),
        Shard::Pawn => latest_base!(pawn),
        // Cold cells (tile/thing) are cold-row-addressed with no per-entity slot — their base is the
        // baseline, read/written by the shards' own reducers.
        _ => None,
    }
}
