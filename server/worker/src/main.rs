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

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Duration;

use spacetimedb_sdk::{DbContext, Table as _};

use resonantdust_codec::action::{
    self, Route, BUILD_WALL, CANCEL_INTENT, CREATE, EXECUTE_INTERACTION, GRANT_CONDITION,
    INIT_ZONE, MOVE_STEP, MOVE_TO, PLACE, PROMOTE, PROMOTE_EVENT, QUEUE_STATE, SET, SET_NEED,
};
use resonantdust_codec::object::{
    cold_row_layer_id, cold_row_macro_position, cold_row_subtype, data_rotation, def_kind_id,
    def_type_id, kind_pos_ref_data, kind_pos_ref_kind_reference, kind_pos_ref_tile,
    pack_cold_row_reference, pack_kind_reference, pack_pawn_data, pack_position_reference,
    pawn_trip_serial, position_macro, position_micro, position_to_tile, tile_to_position,
    TYPE_BIOME_THING, TYPE_BIOME_TILE, TYPE_PAWN,
};
use resonantdust_codec::speed;
use resonantdust_codec::refs::entity_ref_type_id;
use resonantdust_codec::status::{status_phase, EVENT_ASSIGNED, EVENT_COMPLETE};
use resonantdust_codec::tic::{tic_add, tic_after, tic_before};
use resonantdust_st_bindings::{data_shard, event_shard, index, pawn, thing, tile};
use resonantdust_uplink::acquire;
use data_shard::{write as _, EntityStateLogTableAccess as _};
use pawn::{grant_condition as _, set_need as _, spawn as _, write as _, EntityStateLogTableAccess as _};
// interactions P3 / stat-model P3: the execute arm reads the pawn shard's COMPOSED rows
// (position + payload traits/conditions + the `needs` sub-table) and the tile shard's
// baseline ⊕ overlay for the F8 on-tile check.
use pawn::{EntityStateTableAccess as _, NeedsTableAccess as _, PayloadTableAccess as _};
use tile::{EntityStateTableAccess as _, OverlayTableAccess as _};
use event_shard::{complete as _, queue_at as _, EventLogTableAccess as _};
use index::MasterClockTableAccess as _;
// P4: the worker also composes cold rows — the **baseline** (`INIT_ZONE` → `write`) and the **overlay**
// (`SET` → `write_overlay`, reading the `overlay_log` base).
use tile::{write as _, write_overlay as _, OverlayLogTableAccess as _};
use thing::{write as _, write_overlay as _, OverlayLogTableAccess as _};
// lumberjack I1: the interaction arm's THING-carrier offer probe reads the composed tiers.
use thing::{EntityStateTableAccess as _, OverlayTableAccess as _};
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

/// The corpus, loaded once at startup — the execute arm resolves interactions/needs/
/// affordances from it (interactions P3) and the movement chain derives `ground_speed`
/// from it (input-rework F8; the per-kind speeds table is GONE with the `speed` field).
/// Reads through THE shared `read_content_dir` (toml-content P6). Gameplay refs resolve
/// through the SEED fallback here, which the allocator-agreement test pins equal to the
/// registry.
fn load_corpus(root: &str) -> Result<resonantdust_content::loader::Bundle, String> {
    let sources = resonantdust_content::content::read_content_dir(std::path::Path::new(root))
        .map_err(|e| format!("read {root}: {e}"))?;
    if sources.is_empty() {
        return Err(format!("no content sources under {root}"));
    }
    resonantdust_content::loader::load(&sources)
        .map_err(|errs| format!("corpus load: {} error(s), first: {:?}", errs.len(), errs.first()))
}

/// The CREATE sidecars a def implies (stat-model F11): TRAIT payload entries + full-value
/// packed need rows, composed HERE from the corpus — the pawn module holds none.
fn mint_sidecars(bundle: &resonantdust_content::loader::Bundle, def: u32) -> (Vec<u32>, Vec<u32>) {
    let kind = def_kind_id(def);
    let mut trait_words = Vec::new();
    for (name, level) in bundle.thing_traits(kind) {
        if let Some(r) = bundle.gameplay_reference("trait", &name) {
            trait_words.extend_from_slice(&resonantdust_codec::payload::trait_entry(
                resonantdust_codec::object::pack_gameplay_row(r, level),
            ));
        }
    }
    let mut need_rows = Vec::new();
    for nref in bundle.thing_needs(kind) {
        if let Some(np) = bundle.need_params_by_ref(nref) {
            let q = resonantdust_codec::value::quantize(np.max as f32, np.min as f32, np.max as f32);
            need_rows.push(resonantdust_codec::object::pack_gameplay_row(nref, q));
        }
    }
    (trait_words, need_rows)
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
    /// The base row's write tic — the active chord's START time (chord-movement F3:
    /// the mid-chord resolve interpolates from here). `0` for a never-written entity.
    base_tic: u16,
}

/// The re-anchor cadence in tics (chord-movement F4/F8): hops fire — and PROMOTE — at
/// most this far apart, so observer drift without a start anchor stays bounded.
const REANCHOR_TICS: f64 = 32.0;
/// The chord-segment cap in tiles (chord-movement I7): bounds a single hop's window.
const CHORD_CAP_TILES: f64 = 8.0;

/// One hop's stride in tiles (chord-movement F8): the re-anchor cadence translated to
/// distance at this pawn's pace, clamped to [1, CHORD_CAP_TILES].
fn hop_stride_tiles(tics_per_tile: f64) -> f64 {
    (REANCHOR_TICS / tics_per_tile.max(1.0)).clamp(1.0, CHORD_CAP_TILES)
}

/// What movement composition needs beyond scratch (chord-movement F2/F3): the pathability
/// probe, the pawn's ACTIVE walk `(obj, stamped_serial) → dest` — read from the OLD
/// chain's still-PENDING hop event, the durable proof the pawn is actually mid-walk (a
/// resting pawn has no pending hop and resolves to its stored row; seen live: reading
/// the NEW order's dest instead teleported a rested pawn a whole chord) — and the pace.
struct ApplyCtx<'a> {
    pathable: &'a dyn Fn(i32, i32) -> bool,
    active_dest: &'a dyn Fn(u32, u8) -> Option<u32>,
    tics_per_tile: &'a dyn Fn(u32) -> f64,
    now: u16,
}

/// The mid-chord RESOLVE (chord-movement F3): where a WALKING pawn actually is at `now` —
/// its stored row is the chord's start, so interpolate `elapsed / tics_per_tile` tiles
/// along the deterministically-recomputed first chord, quantized to subtile. A pawn with
/// no active walk (or an unroutable one) resolves to its stored position. The half-window
/// guard reads a future-stamped row as elapsed 0, never ancient (the needs-eval rule).
fn resolve_walk_position_for(obj: u32, p: &Payload, ctx: &ApplyCtx) -> u32 {
    use resonantdust_codec::object::{point_to_position, position_to_point};
    let Some(dest) = (ctx.active_dest)(obj, pawn_trip_serial(p.data)) else {
        return p.position_reference;
    };
    let start_tile = position_to_tile(p.position_reference);
    let dest_tile = position_to_tile(dest);
    if start_tile == dest_tile {
        return p.position_reference;
    }
    let Some(chords) =
        resonantdust_content::path_eval::find_chords(start_tile, dest_tile, 0, ctx.pathable)
    else {
        return p.position_reference;
    };
    let Some(&(wx, wy)) = chords.first() else { return p.position_reference };
    let raw = ctx.now.wrapping_sub(p.base_tic);
    let elapsed = if raw > u16::MAX / 2 { 0 } else { raw } as f64;
    let pace = (ctx.tics_per_tile)(obj);
    if pace < 1.0 {
        return p.position_reference;
    }
    let walked = elapsed / pace;
    let (sx, sy) = position_to_point(p.position_reference);
    let (dxf, dyf) = (wx as f64 - sx, wy as f64 - sy);
    let len = dxf.hypot(dyf);
    if len <= f64::EPSILON {
        return p.position_reference;
    }
    let f = (walked / len).min(1.0);
    point_to_position(sx + dxf * f, sy + dyf * f)
}

/// One parked order in a pawn's EPHEMERAL intent queue (lumberjack F1 — ACTIONS.md
/// § The intent queue): re-queued as a version-2 (ADVANCED) `EXECUTE_INTERACTION` event
/// when the intent ahead of it completes. The DURABLE half is always the queued event
/// itself; a worker bounce loses only this pending tail, and execution-time
/// re-validation turns any stale survivor into a logged no-op.
struct PendingIntent {
    /// The worker-minted DISPLAY identity (intent-queue-ui) — what a cancel click names.
    entry_id: u32,
    interaction_ref: u32,
    inputs: Vec<u32>,
}

/// What a pawn's queue is waiting on (the in-flight half).
enum Running {
    /// A composed walk — completes when the pawn's authoritative row reaches `dest`.
    /// No trip-serial check needed: every seed flows through the interaction arm, and a
    /// fresh order REPLACES the whole queue before its new seed supersedes the chain
    /// (F3) — a superseded chain's queue is already gone.
    Move { entry_id: u32, interaction_ref: u32, dest: u32 },
    /// A `duration > 0` intent — completes when its version-1 COMPLETION event fires.
    Timed { entry_id: u32, interaction_ref: u32, started_tic: u16, fire_tic: u16 },
}

struct PawnQueue {
    pending: VecDeque<PendingIntent>,
    running: Option<Running>,
    /// The entry carried across an ADVANCEMENT: popped from `pending` when its
    /// version-2 order is queued, consumed when that order schedules/executes — so the
    /// DISPLAY identity survives the re-queue (intent-queue-ui).
    advancing: Option<(u32, u32)>, // (entry_id, interaction_ref)
}

impl PawnQueue {
    fn new() -> Self {
        PawnQueue { pending: VecDeque::new(), running: None, advancing: None }
    }

    /// The `QUEUE_STATE` fan program (intent-queue-ui F1): entry 0 = the ACTIVE event,
    /// pending after, each `[entry_id, interaction_ref, phase, started:16|fire:16]`.
    /// Phase: 0 pending / 1 walking / 2 executing (ACTIONS.md palette). An entry mid-
    /// advancement fans as pending (phase 0, its next fan corrects it).
    fn fan_program(&self, pawn: u32) -> Vec<u32> {
        let mut entries: Vec<u32> = Vec::new();
        match &self.running {
            Some(Running::Move { entry_id, interaction_ref, .. }) => {
                entries.extend_from_slice(&[*entry_id, *interaction_ref, 1, 0]);
            }
            Some(Running::Timed { entry_id, interaction_ref, started_tic, fire_tic }) => {
                entries.extend_from_slice(&[
                    *entry_id,
                    *interaction_ref,
                    2,
                    (u32::from(*started_tic) << 16) | u32::from(*fire_tic),
                ]);
            }
            None => {}
        }
        if let Some((entry_id, interaction_ref)) = &self.advancing {
            entries.extend_from_slice(&[*entry_id, *interaction_ref, 0, 0]);
        }
        for p in &self.pending {
            entries.extend_from_slice(&[p.entry_id, p.interaction_ref, 0, 0]);
        }
        let mut prog = vec![PROMOTE_EVENT, QUEUE_STATE, pawn, 0, entries.len() as u32];
        prog.extend_from_slice(&entries);
        prog
    }
}

/// The queue-depth law (lumberjack F3): a composition that would exceed this rejects WHOLE.
const INTENT_CAP: usize = 5;

/// `EXECUTE_INTERACTION`'s version word, repurposed as the intent-queue discriminator
/// (lumberjack): clients always send FRESH. Dev posture: the edge doesn't police the
/// word (no ownership model) — a hand-rolled completion just re-validates like any other.
const INTENT_FRESH: u32 = 0;
const INTENT_COMPLETION: u32 = 1;
const INTENT_ADVANCED: u32 = 2;

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

    // The corpus, once at startup (the sim container mounts the repo). Speed is now the
    // DERIVED `ground_speed` stat (input-rework F8) — evaluated per hop from each pawn's
    // fanned rows through this bundle. A missing/broken corpus is a MISCONFIG, not an
    // outage — running anyway would step every pawn at the default while clients speculate
    // derived speeds, so exit for the restart policy instead of drifting.
    let content_dir = env_or("CONTENT_DIR", "/workspace/content");
    let bundle = match load_corpus(&content_dir) {
        Ok(b) => {
            tracing::info!(%content_dir, kinds = b.thing_names().len(),
                interactions = b.interaction_names().len(), "corpus loaded");
            b
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
    // chord-movement F3: `worker_reference = 0` (queued, not yet assigned) rides along so
    // the mid-chord resolve can see the OLD chain's PENDING hop — the durable proof a pawn
    // is mid-walk. Dev-scale acceptable, the same every-row posture the tile mirror takes.
    let event_up = resonantdust_uplink::subbed_uplink!(event_shard, "event_shard", uri, event_db,
        vec![format!("SELECT * FROM event_log WHERE worker_reference = {self_ref} OR worker_reference = 0")]);
    let hot_sql = |_: ()| format!("SELECT * FROM entity_state_log WHERE worker_reference = {self_ref} OR observer_reference = {self_ref}");
    let data_up = resonantdust_uplink::subbed_uplink!(data_shard, "data_shard", uri, data_db, vec![hot_sql(())]);
    // The pawn shard adds its COMPOSED public rows (interactions P3): `EXECUTE_INTERACTION`
    // claims nothing (its writes ride the verbs it queues), so the execute arm reads the
    // target's position + payload here rather than through a claim. The shard holds pawns
    // only, so "all rows" is small by construction.
    let pawn_up = resonantdust_uplink::subbed_uplink!(pawn, "pawn", uri, pawn_db,
        vec![hot_sql(()), "SELECT * FROM entity_state".to_string(), "SELECT * FROM payload".to_string(),
             "SELECT * FROM needs".to_string()]);
    let cold_sql: Vec<String> = ["entity_state_log", "overlay_log"]
        .iter()
        .map(|t| format!("SELECT * FROM {t} WHERE worker_reference = {self_ref} OR observer_reference = {self_ref}"))
        .collect();
    // The tile shard likewise adds its composed baseline + overlay (interactions P3/F8): the
    // "on"-tile check reads the kind under the pawn. EVERY zone row mirrors here — acceptable
    // at dev scale, and the per-zone worker state tree-occupancy plans will scope it.
    let mut tile_sql = cold_sql.clone();
    tile_sql.push("SELECT * FROM entity_state".to_string());
    tile_sql.push("SELECT * FROM overlay".to_string());
    let tile_up = resonantdust_uplink::subbed_uplink!(tile, "tile", uri, tile_db, tile_sql);
    // The thing shard mirrors the tile's composed tiers (lumberjack I1): the interaction
    // arm's THING-carrier offer probe (a tree offering cut_down) reads them.
    let mut thing_sql = cold_sql.clone();
    thing_sql.push("SELECT * FROM entity_state".to_string());
    thing_sql.push("SELECT * FROM overlay".to_string());
    let thing_up = resonantdust_uplink::subbed_uplink!(thing, "thing", uri, thing_db, thing_sql);

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

    // The per-pawn intent queues (lumberjack F1) — worker MEMORY, deliberately: the
    // in-flight order is a durable queued event; this map is only the tail behind it.
    let mut intent_queues: HashMap<u32, PawnQueue> = HashMap::new();
    // The DISPLAY identity mint (intent-queue-ui) — never 0, ephemeral like the queue.
    let mut intent_entry_seq: u32 = 0;
    // Cancelled EXECUTING intents (intent-queue-ui F3): `(pawn, fire_tic)` — consumed
    // when the completion arrives, turning it into a logged `cancelled` NO-OP. Worker
    // memory: a bounce forgets it and a still-valid completion executes (stated in
    // ACTIONS.md).
    let mut cancelled: HashSet<(u32, u16)> = HashSet::new();

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

        // ── composed-tier kind probes (lumberjack I1, HOISTED by pathfinding F2) ── the
        // interaction arm's carrier resolution AND the movement chain's pathability read
        // the SAME view. `tile_kind_at`: a zone's ground is one baseline row per biome
        // subtype (a nonzero cell wins), overlays override. `thing_kind_at`: sparse
        // baseline ⊕ overlay where an overlay kind 0 SUPPRESSES (a felled tree neither
        // offers nor blocks); returns the owning cold_row for the destroy composer.
        let tile_kind_at = |zone: u16, cell: u8| -> Option<u16> {
            let mut k = tile
                .db()
                .entity_state()
                .iter()
                .filter(|r| r.macro_position_reference == zone)
                .find_map(|r| {
                    r.items
                        .get(usize::from(cell))
                        .map(|i| i.kind_reference)
                        .filter(|&k| k != 0)
                });
            for o in tile.db().overlay().iter().filter(|r| r.macro_position_reference == zone) {
                for it in &o.items {
                    if it.tile_reference == cell && it.kind_reference != 0 {
                        k = Some(it.kind_reference);
                    }
                }
            }
            k
        };
        let thing_kind_at = |zone: u16, cell: u8| -> Option<(u16, u32)> {
            let mut k = thing
                .db()
                .entity_state()
                .iter()
                .filter(|r| r.macro_position_reference == zone)
                .find_map(|r| {
                    r.items
                        .iter()
                        .find(|i| i.tile_reference == cell && i.kind_reference != 0)
                        .map(|i| (i.kind_reference, r.cold_row_reference))
                });
            for o in thing.db().overlay().iter().filter(|r| r.macro_position_reference == zone) {
                for it in &o.items {
                    if it.tile_reference == cell {
                        k = (it.kind_reference != 0)
                            .then_some((it.kind_reference, o.cold_row_reference));
                    }
                }
            }
            k
        };
        // ── the ONE pathability probe (pathfinding F1) ── a cell is enterable iff its
        // tile kind AND its occupant agree; kind 0 / an unknown zone reads OPEN (version
        // skew degrades to ground, never an invisible wall — the Bundle carries the same
        // guard). Every hop, seed check, and walk-dest pick this pass asks HERE.
        let cell_pathable = |x: i32, y: i32| -> bool {
            let pos = tile_to_position(x, y);
            let zone = position_macro(pos);
            let cell = (position_micro(pos) >> 8) as u8;
            tile_kind_at(zone, cell).is_none_or(|k| bundle.tile_pathable(k >> 4))
                && thing_kind_at(zone, cell).is_none_or(|(k, _)| bundle.thing_pathable(k >> 4))
        };

        // Hop spacing is the pawn's DERIVED `ground_speed` (input-rework F8): evaluated
        // per hop from its fanned rows, so a mid-trip condition that slows a stat slows
        // the chain. The default is the degenerate fallback only — the `can_move_ground`
        // gate refuses a 0-derived pawn at the front door. (Hoisted to pass level by
        // chord-movement: the compose's resolves need it too.)
        let ground_speed_tics = |entity: u32, now: u16| -> u16 {
            let (trait_rows, cond_rows) = pawn
                .db()
                .payload()
                .iter()
                .find(|r| r.entity_reference == entity)
                .map(|r| {
                    (
                        resonantdust_codec::payload::payload_traits(&r.payload),
                        resonantdust_codec::payload::payload_conditions(&r.payload),
                    )
                })
                .unwrap_or_default();
            let need_rows: Vec<(u32, u16)> = pawn
                .db()
                .needs()
                .iter()
                .filter(|r| r.entity_reference == entity)
                .map(|r| (r.need, r.set_tic))
                .collect();
            let active = resonantdust_content::needs_eval::active_conditions(
                &bundle, &trait_rows, &need_rows, &cond_rows, now,
            );
            let v = resonantdust_content::stat_eval::stat_value(
                &bundle, "ground_speed", &trait_rows, &active,
            );
            if v >= 1.0 { v.round() as u16 } else { speed::DEFAULT_TICS_PER_TILE }
        };

        // ── INTENT ADVANCEMENT, move half (lumberjack I4) ── a Move-running queue head
        // completes when the pawn's authoritative row reaches its dest; completing an
        // event queues the NEXT one automatically (the user's law, F1). Runs BEFORE the
        // empty-pass early-out — arrival often lands on a pass with no assigned events.
        let arrived: Vec<u32> = intent_queues
            .iter()
            .filter_map(|(&p, q)| match q.running {
                Some(Running::Move { dest, .. }) => {
                    pawn.db().entity_state().iter().find(|r| r.entity_reference == p).and_then(
                        |r| {
                            let pos = pack_position_reference(
                                r.macro_position_reference,
                                r.micro_position_reference,
                            );
                            (position_to_tile(pos) == position_to_tile(dest)).then_some(p)
                        },
                    )
                }
                _ => None,
            })
            .collect();
        for p in arrived {
            let Some(q) = intent_queues.get_mut(&p) else { continue };
            q.running = None;
            if let Some(next) = q.pending.pop_front() {
                let mut prog = vec![
                    EXECUTE_INTERACTION,
                    next.interaction_ref,
                    INTENT_ADVANCED,
                    next.inputs.len() as u32,
                ];
                prog.extend_from_slice(&next.inputs);
                // The display identity survives the re-queue (intent-queue-ui).
                q.advancing = Some((next.entry_id, next.interaction_ref));
                match event.reducers().queue_at(prog, tic_add(master, 4)) {
                    Ok(()) => tracing::info!(master, pawn = format!("{p:#010x}"),
                        pending = q.pending.len(), "intent advanced — walk landed, next order queued"),
                    Err(err) => tracing::warn!(%err, pawn = format!("{p:#010x}"),
                        "intent advance queue failed — pending tail dropped"),
                }
            }
            let _ = event.reducers().queue_at(q.fan_program(p), tic_add(master, 4));
            if q.pending.is_empty() && q.running.is_none() && q.advancing.is_none() {
                intent_queues.remove(&p);
            }
        }

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
                        base_tic: r.tic,
                    })
                    .unwrap_or_default();
                scratch.insert(e, base);
            }
            let mut promote: HashSet<u32> = HashSet::new();
            {
                // The compose's movement context (chord-movement F3): the resolve reads
                // the ACTIVE walk from the OLD chain's still-PENDING hop event — durable,
                // and a resting pawn (no pending hop with its stamped serial) resolves to
                // its stored row.
                let active_dest = |obj: u32, serial: u8| -> Option<u32> {
                    event.db().event_log().iter().find_map(|e| {
                        if status_phase(e.status) == EVENT_COMPLETE {
                            return None;
                        }
                        for inst in action::program(&e.actions) {
                            let Ok(inst) = inst else { break };
                            if let (MOVE_STEP, [o, d, s]) = (inst.action, inst.operands) {
                                if *o == obj && (*s & 0x3F) as u8 == serial {
                                    return Some(*d);
                                }
                            }
                        }
                        None
                    })
                };
                let tics_per_tile = |obj: u32| -> f64 { f64::from(ground_speed_tics(obj, t)) };
                let ctx = ApplyCtx {
                    pathable: &cell_pathable,
                    active_dest: &active_dest,
                    tics_per_tile: &tics_per_tile,
                    now: t,
                };
                for (event_reference, actions) in &events {
                    apply(actions, &mut scratch, &mut promote, *event_reference, &ctx);
                }
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
                            // Variable arity (human-pawns F2): def, position, count, payload×count.
                            if let [def, pos, _count, payload @ ..] = inst.operands {
                                // CREATE routes by the packed def's type (F2). Only the TYPE_PAWN
                                // arm exists; any other type is REJECTED BY NAME, never silently
                                // pawned — future arms (e.g. TYPE_THING mints) slot in here. The
                                // index still advances so a mixed program's ledger keys are stable.
                                if def_type_id(*def) != TYPE_PAWN {
                                    // No `continue` — the loop tail must still clear the
                                    // pending PROMOTE latch this instruction consumed.
                                    tracing::warn!(
                                        def = format!("{def:#010x}"),
                                        type_id = def_type_id(*def),
                                        tic = t,
                                        "CREATE rejected: no shard arm for this def type (only TYPE_PAWN is implemented)"
                                    );
                                } else {
                                    // stat-model F11: the def's starting traits ride the
                                    // payload as TRAIT entries; its needs mint full-value
                                    // rows in the `needs` sub-table — both composed here
                                    // (the module holds no corpus).
                                    let (trait_words, need_rows) = mint_sidecars(&bundle, *def);
                                    let mut words = payload.to_vec();
                                    words.extend_from_slice(&trait_words);
                                    if let Err(err) = pawn.reducers().spawn(
                                        self_ref,
                                        t,
                                        *event_reference,
                                        index,
                                        *def,
                                        *pos,
                                        words,
                                        need_rows,
                                        pending_promote,
                                    ) {
                                        tracing::warn!(%err, tic = t, "spawn failed — will retry next pass");
                                        spawn_failed = true;
                                    }
                                    create_zones.entry(*event_reference).or_default().push(position_macro(*pos));
                                }
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

            // ── NEEDS (SET_NEED / GRANT_CONDITION) ── payload-entry verbs (needs-moodlets F7):
            // the PAWN MODULE composes the splice (read current payload → upsert → write),
            // so this arm only relays. Idempotent by content at this tic, so a failed call
            // defers the whole tic (like a write) and the re-pass re-calls harmlessly.
            let mut need_failed = false;
            for (_event_reference, actions) in &events {
                for inst in action::program(actions) {
                    let Ok(inst) = inst else { break };
                    let r = match (inst.action, inst.operands) {
                        // stat-model: both verbs carry ONE packed gameplay row beside the obj.
                        (SET_NEED, [obj, row]) => pawn.reducers().set_need(self_ref, t, *obj, *row),
                        (GRANT_CONDITION, [obj, row]) => pawn.reducers().grant_condition(self_ref, t, *obj, *row),
                        _ => continue,
                    };
                    if let Err(err) = r {
                        tracing::warn!(%err, tic = t, "need/condition write failed — will retry next pass");
                        need_failed = true;
                    }
                }
            }
            if need_failed {
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
            //
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
                    if (cx, cy) == (tx, ty) && p.position_reference == dest {
                        continue; // arrived — the chain ends
                    }
                    // The next hop lands one STRIDE along the first chord (chord-movement
                    // F2/F6/F8): `k = ceil(dist × tics_per_tile)`, dist = min(chord len,
                    // the re-anchor stride). EVERY hop PROMOTEs — hops fire AT the
                    // re-anchor cadence, so each write IS the drift-bounding anchor (F4;
                    // fan ≤ one frame per REANCHOR_TICS per moving pawn, never per tile).
                    // A blocked route queues a BARE retry — the chain idles, re-routing
                    // every fire, and resumes if the blockage clears.
                    let pace = f64::from(ground_speed_tics(obj, t));
                    let (program, k) = match resonantdust_content::path_eval::find_chords(
                        (cx, cy),
                        (tx, ty),
                        0,
                        &cell_pathable,
                    ) {
                        Some(chords) if !chords.is_empty() => {
                            use resonantdust_codec::object::position_to_point;
                            let (w0x, w0y) = chords[0];
                            let (pxf, pyf) = position_to_point(p.position_reference);
                            let len = (w0x as f64 - pxf).hypot(w0y as f64 - pyf);
                            let dist = len.min(hop_stride_tiles(pace));
                            let k = (dist * pace).ceil().max(4.0) as u16;
                            (vec![PROMOTE, MOVE_STEP, obj, dest, serial], k)
                        }
                        _ => (vec![MOVE_STEP, obj, dest, serial], 8),
                    };
                    let next_tic = tic_add(t, k);
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
                    // Beyond the queue_at barrier (master+3), NOT t+1 — the same silent-loss
                    // race the interaction effects hit (interactions P3): a t+1 queue whose
                    // tic the clock has passed is rejected async and the tile just vanishes.
                    let next_tic = tic_add(master, 4);
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

            // ── INTERACTIONS (EXECUTE_INTERACTION interaction version count inputs…) ── resolve
            // the corpus def, validate, and queue the REAL writes (`PROMOTE SET_NEED` /
            // `PROMOTE GRANT_CONDITION`) — the BUILD_WALL pattern (interactions F4): this verb
            // writes nothing itself, so the typed queued verbs carry the write set. A validation
            // failure LOGS AND DROPS THE EVENT WHOLE (I6) — never half-executes.
            for (event_reference, actions) in &events {
                for inst in action::program(actions) {
                    let Ok(inst) = inst else { break };
                    // ── CANCEL_INTENT pawn entry_id (intent-queue-ui F3/F4) ── resolves
                    // against worker MEMORY by phase; every path refans; unknown = no-op.
                    if inst.action == CANCEL_INTENT {
                        let [pawn_ref, entry_id] = inst.operands else { continue };
                        let plabel = format!("{pawn_ref:#010x}");
                        let Some(q) = intent_queues.get_mut(pawn_ref) else {
                            tracing::info!(tic = t, pawn = %plabel, entry_id,
                                "cancel no-op — the pawn holds no queue");
                            continue;
                        };
                        if let Some(i) = q.pending.iter().position(|p| p.entry_id == *entry_id) {
                            q.pending.remove(i);
                            tracing::info!(tic = t, pawn = %plabel, entry_id,
                                "intent cancelled — pending entry removed");
                            let _ = event
                                .reducers()
                                .queue_at(q.fan_program(*pawn_ref), tic_add(master, 4));
                            if q.pending.is_empty() && q.running.is_none() && q.advancing.is_none()
                            {
                                intent_queues.remove(pawn_ref);
                            }
                            continue;
                        }
                        match &q.running {
                            Some(Running::Timed {
                                entry_id: id,
                                interaction_ref: iref,
                                fire_tic,
                                ..
                            }) if id == entry_id => {
                                // Cancelable is the TOML's call, only for the EXECUTING case.
                                let ok = bundle
                                    .gameplay_lookup(*iref)
                                    .and_then(|(_, n)| bundle.interaction_params(&n))
                                    .map(|p| p.queue.cancelable)
                                    .unwrap_or(false);
                                if ok {
                                    cancelled.insert((*pawn_ref, *fire_tic));
                                    q.running = None;
                                    tracing::info!(tic = t, pawn = %plabel, entry_id,
                                        "intent cancelled — executing entry; its completion will NO-OP");
                                    let _ = event
                                        .reducers()
                                        .queue_at(q.fan_program(*pawn_ref), tic_add(master, 4));
                                } else {
                                    tracing::info!(tic = t, pawn = %plabel, entry_id,
                                        "cancel REFUSED — the interaction is not cancelable while executing");
                                }
                            }
                            Some(Running::Move { entry_id: id, .. }) if id == entry_id => {
                                // F3: cancelling the walk clears the queue; the chain
                                // finishes its glide and the parked act dies with it.
                                intent_queues.remove(pawn_ref);
                                tracing::info!(tic = t, pawn = %plabel, entry_id,
                                    "intent cancelled — walk; queue cleared, glide finishes");
                                let _ = event.reducers().queue_at(
                                    PawnQueue::new().fan_program(*pawn_ref),
                                    tic_add(master, 4),
                                );
                            }
                            _ => {
                                tracing::info!(tic = t, pawn = %plabel, entry_id,
                                    "cancel no-op — entry not in the queue (completed or mid-advancement)");
                            }
                        }
                        continue;
                    }
                    let (interaction_ref, version, inputs) = match (inst.action, inst.operands) {
                        (EXECUTE_INTERACTION, [i, v, _count, inputs @ ..]) => (*i, *v, inputs),
                        _ => continue,
                    };
                    // The version word discriminates the intent-queue flows (lumberjack):
                    // FRESH replaces the pawn's queue, ADVANCED continues it, COMPLETION
                    // executes a scheduled `duration` intent's effects (re-validating).
                    let is_completion = version == INTENT_COMPLETION;
                    let reject = |why: &str| {
                        // A dropped COMPLETION is the intent queue's NO-OP (lumberjack
                        // F1) — same law, named in the log so drills can tell them apart.
                        if version == INTENT_COMPLETION {
                            tracing::info!(event = format!("{event_reference:#010x}"), tic = t,
                                why, "intent completion NO-OP");
                        } else {
                            tracing::warn!(event = format!("{event_reference:#010x}"), tic = t,
                                why, "interaction dropped");
                        }
                    };
                    let Some(params) = bundle.interaction_params_by_ref(interaction_ref) else {
                        reject("unknown interaction def");
                        continue;
                    };
                    let (_, iname) = bundle.gameplay_lookup(interaction_ref).expect("resolved above");
                    if inputs.len() != params.inputs.len() {
                        reject("input count does not match the corpus signature");
                        continue;
                    }
                    use resonantdust_content::loader::Operand;
                    use resonantdust_codec::object::{gameplay_row_data, pack_gameplay_row};
                    // The acting PAWN: whichever effect binds it, else the reserved `pawn`
                    // input (input-rework F5) — a destroy-only interaction (cut_down) has
                    // no satisfy/move to name it (lumberjack).
                    let target_op = params
                        .satisfy
                        .as_ref()
                        .map(|s| &s.target)
                        .or(params.move_effect.as_ref().map(|m| &m.target));
                    let target = match target_op {
                        Some(&Operand::Input(ti)) => inputs[ti],
                        _ => match params.inputs.iter().position(|n| n == "pawn") {
                            Some(i) => inputs[i],
                            None => {
                                reject("no effect or `pawn` input names the acting pawn");
                                continue;
                            }
                        },
                    };
                    // A FRESH order REPLACES the pawn's whole queue (F3 — the one-chain
                    // law lifted a level); preemption IS a new order. The strip clears
                    // via an EMPTY fan (intent-queue-ui).
                    if version == INTENT_FRESH && intent_queues.remove(&target).is_some() {
                        tracing::info!(tic = t, pawn = format!("{target:#010x}"),
                            "intent queue REPLACED by a fresh order (F3)");
                        let _ = event
                            .reducers()
                            .queue_at(PawnQueue::new().fan_program(target), tic_add(master, 4));
                    }
                    // A COMPLETION advances the queue ON RECEIPT — the outcome (execute
                    // or no-op) never retries and never blocks the next intent (F1).
                    if is_completion {
                        if let Some(q) = intent_queues.get_mut(&target) {
                            if matches!(q.running, Some(Running::Timed { fire_tic, .. }) if fire_tic == t)
                            {
                                q.running = None;
                                if let Some(next) = q.pending.pop_front() {
                                    let mut prog = vec![
                                        EXECUTE_INTERACTION,
                                        next.interaction_ref,
                                        INTENT_ADVANCED,
                                        next.inputs.len() as u32,
                                    ];
                                    prog.extend_from_slice(&next.inputs);
                                    q.advancing = Some((next.entry_id, next.interaction_ref));
                                    match event.reducers().queue_at(prog, tic_add(master, 4)) {
                                        Ok(()) => tracing::info!(tic = t,
                                            pawn = format!("{target:#010x}"),
                                            "intent advanced — completion fired, next order queued"),
                                        Err(err) => tracing::warn!(%err,
                                            pawn = format!("{target:#010x}"),
                                            "intent advance queue failed — pending tail dropped"),
                                    }
                                }
                                let _ = event
                                    .reducers()
                                    .queue_at(q.fan_program(target), tic_add(master, 4));
                                if q.pending.is_empty()
                                    && q.running.is_none()
                                    && q.advancing.is_none()
                                {
                                    intent_queues.remove(&target);
                                }
                            }
                        }
                        // A CANCELLED completion is the click's promised outcome
                        // (intent-queue-ui F3): consume the marker and no-op WHOLE.
                        if cancelled.remove(&(target, t)) {
                            reject("cancelled by CANCEL_INTENT (intent-queue-ui F3)");
                            continue;
                        }
                    }
                    // The destination (a position_reference): the move effect's `to`, else
                    // the reserved `destination` input — adjacency-gated interactions name
                    // their carrier through it without moving (lumberjack).
                    let dest = params
                        .move_effect
                        .as_ref()
                        .and_then(|m| match &m.to {
                            Operand::Input(i) => Some(inputs[*i]),
                            _ => None,
                        })
                        .or_else(|| {
                            params.inputs.iter().position(|n| n == "destination").map(|i| inputs[i])
                        });
                    // The target must be a live pawn — its composed row anchors the location.
                    let Some(prow) =
                        pawn.db().entity_state().iter().find(|r| r.entity_reference == target)
                    else {
                        reject("target is not a live pawn");
                        continue;
                    };
                    // The CARRIER candidates by the location rule (input-rework F4 /
                    // lumberjack F2): "on" = the pawn's own cell; "target" = the
                    // destination's; "adjacent" with a bound destination = that cell,
                    // pawn within Chebyshev ≤ 1 INCLUSIVE — without one (drink), ANY
                    // cell of the pawn's 3×3 may carry the offerer.
                    let pawn_pos = pack_position_reference(
                        prow.macro_position_reference,
                        prow.micro_position_reference,
                    );
                    // chord-movement F3/I2: a WALKING pawn validates from its RESOLVED
                    // mid-chord position, never the stale chord start — then floors.
                    let pawn_pos = {
                        let pp = Payload {
                            definition_reference: prow.definition_reference,
                            position_reference: pawn_pos,
                            data: prow.data,
                            base_tic: prow.tic,
                        };
                        let active_dest = |obj: u32, serial: u8| -> Option<u32> {
                            event.db().event_log().iter().find_map(|e| {
                                if status_phase(e.status) == EVENT_COMPLETE {
                                    return None;
                                }
                                for inst in action::program(&e.actions) {
                                    let Ok(inst) = inst else { break };
                                    if let (MOVE_STEP, [o, d, s]) = (inst.action, inst.operands) {
                                        if *o == obj && (*s & 0x3F) as u8 == serial {
                                            return Some(*d);
                                        }
                                    }
                                }
                                None
                            })
                        };
                        let pace = |obj: u32| -> f64 { f64::from(ground_speed_tics(obj, t)) };
                        let rctx = ApplyCtx {
                            pathable: &cell_pathable,
                            active_dest: &active_dest,
                            tics_per_tile: &pace,
                            now: t,
                        };
                        resolve_walk_position_for(target, &pp, &rctx)
                    };
                    let (px, py) = position_to_tile(pawn_pos);
                    // Out of place by DISTANCE alone → the intent queue composes
                    // [walk, act] instead of rejecting (lumberjack; ACTIONS.md § The
                    // intent queue). Set here, consumed after the affordance gate — a
                    // pawn that could never act must not walk first.
                    let mut needs_walk = false;
                    let candidates: Vec<u32> = match params.location.as_str() {
                        "on" => vec![pawn_pos],
                        "target" => match dest {
                            Some(d) => vec![d],
                            None => {
                                reject("location `target` needs a destination input");
                                continue;
                            }
                        },
                        "adjacent" => match dest {
                            Some(d) => {
                                let (dx, dy) = position_to_tile(d);
                                let cheb = px.abs_diff(dx).max(py.abs_diff(dy));
                                if !resonantdust_content::loader::location_in_range(
                                    "adjacent", cheb,
                                ) {
                                    if is_completion {
                                        reject("completion NO-OP: the pawn left the carrier's range (lumberjack F1)");
                                        continue;
                                    }
                                    needs_walk = true;
                                }
                                vec![d]
                            }
                            None => {
                                let mut v = Vec::with_capacity(9);
                                for oy in -1i32..=1 {
                                    for ox in -1i32..=1 {
                                        v.push(tile_to_position(px + ox, py + oy));
                                    }
                                }
                                v
                            }
                        },
                        _ => {
                            reject("unknown location rule");
                            continue;
                        }
                    };
                    // The offer probe (lumberjack I1): `tile_kind_at`/`thing_kind_at` are
                    // the pass-level composed-tier probes (hoisted by pathfinding F2 —
                    // the movement chain reads the same view).
                    // The first candidate whose def OFFERS this interaction wins
                    // (stat-model F9 binding; walls not carrying move_to is exactly this
                    // refusal — input-rework F9). Things probe first: a tree stands ON
                    // grass, and the thing is the more specific carrier.
                    // (position, is_thing, owning cold_row — 0 for tiles, kind_reference —
                    // the destroy composer resolves the BINDING's yield through it).
                    let mut carrier: Option<(u32, bool, u32, u16)> = None;
                    for &c in &candidates {
                        let zone = position_macro(c);
                        let cell = (position_micro(c) >> 8) as u8;
                        if let Some((kr, row)) = thing_kind_at(zone, cell) {
                            if bundle
                                .thing_interactions(kr >> 4)
                                .iter()
                                .any(|b| b.name == iname)
                            {
                                carrier = Some((c, true, row, kr));
                                break;
                            }
                        }
                        if let Some(kr) = tile_kind_at(zone, cell) {
                            if bundle.tile_interactions(kr >> 4).iter().any(|b| b.name == iname) {
                                carrier = Some((c, false, 0, kr));
                                break;
                            }
                        }
                    }
                    let Some(carrier) = carrier else {
                        reject("no carrier in range offers this interaction (F4/F2 location rule)");
                        continue;
                    };
                    // The pawn's gameplay rows: traits + stored conditions from the payload
                    // sidecar, needs from the `needs` sub-table (stat-model F1/F2).
                    let (trait_rows, cond_rows) = pawn
                        .db()
                        .payload()
                        .iter()
                        .find(|r| r.entity_reference == target)
                        .map(|r| {
                            (
                                resonantdust_codec::payload::payload_traits(&r.payload),
                                resonantdust_codec::payload::payload_conditions(&r.payload),
                            )
                        })
                        .unwrap_or_default();
                    let need_rows: Vec<(u32, u16)> = pawn
                        .db()
                        .needs()
                        .iter()
                        .filter(|r| r.entity_reference == target)
                        .map(|r| (r.need, r.set_tic))
                        .collect();
                    // The effect must queue BEYOND the event shard's TIC_GAP barrier (queue_at
                    // rejects anything nearer than master+3, and the SDK error is async —
                    // invisible here). t+1 raced the clock and LOST ~half the sips (seen live,
                    // interactions P3); the movement chains never hit this because they queue
                    // tics_per_tile out. Satisfaction is computed AT the target tic so the
                    // stored row is self-consistent.
                    let effect_tic = tic_add(master, 4);
                    // The affordance PREDICATE gate (stat-model F5/F8): every gate on the
                    // interaction must pass for the target's DERIVED stats — the same
                    // `interaction_available` the npc asks, so the two cannot disagree (I2).
                    let active = resonantdust_content::needs_eval::active_conditions(
                        &bundle, &trait_rows, &need_rows, &cond_rows, effect_tic,
                    );
                    if !resonantdust_content::stat_eval::interaction_available(
                        &bundle, &iname, &trait_rows, &active,
                    ) {
                        reject("an affordance predicate fails for the target (stat-model F5)");
                        continue;
                    }
                    // ── COMPOSE (lumberjack) ── out of place, gates passed: walk to the
                    // carrier, park THIS order behind the walk. The seed is its own
                    // durable event; the parked order re-enters as version-2 on arrival.
                    if needs_walk {
                        let dc = dest.expect("needs_walk only sets with a bound destination");
                        // The walk stops at the nearest PATHABLE cell within the location
                        // rule's range of the carrier (pathfinding I4): drink at water
                        // must land on the SHORELINE, never in the lake. Deterministic
                        // pick — shortest shared path, ties by (y, x). None reachable →
                        // the whole order no-ops (F5).
                        let (dx, dy) = position_to_tile(dc);
                        let mut best: Option<(usize, i32, i32)> = None;
                        for oy in -1i32..=1 {
                            for ox in -1i32..=1 {
                                let (nx, ny) = (dx + ox, dy + oy);
                                let plen = if (nx, ny) == (px, py) {
                                    Some(0)
                                } else {
                                    resonantdust_content::path_eval::path_len(
                                        (px, py),
                                        (nx, ny),
                                        &cell_pathable,
                                    )
                                };
                                if let Some(l) = plen {
                                    let cand = (l, ny, nx);
                                    if best.is_none_or(|b| cand < b) {
                                        best = Some(cand);
                                    }
                                }
                            }
                        }
                        let Some((_, by, bx)) = best else {
                            reject("no pathable cell in the carrier's range is reachable (pathfinding F5)");
                            continue;
                        };
                        let d = tile_to_position(bx, by);
                        let q = intent_queues.entry(target).or_insert_with(PawnQueue::new);
                        if q.pending.len() >= INTENT_CAP {
                            reject("intent queue full (cap 5, F3) — order rejected whole");
                            continue;
                        }
                        // Two DISPLAY entries from one order (intent-queue-ui): the walk
                        // and the parked act each mint their own id.
                        intent_entry_seq += 2;
                        let walk_id = intent_entry_seq - 1;
                        let act_id = intent_entry_seq;
                        let move_ref =
                            bundle.gameplay_reference("interaction", "move_to").unwrap_or(0);
                        q.pending.push_back(PendingIntent {
                            entry_id: act_id,
                            interaction_ref,
                            inputs: inputs.to_vec(),
                        });
                        q.running = Some(Running::Move {
                            entry_id: walk_id,
                            interaction_ref: move_ref,
                            dest: d,
                        });
                        if let Err(err) = event
                            .reducers()
                            // chord-movement F4: the seed fans the INTENT, not the start
                            // position — no PROMOTE prefix on MOVE_TO.
                            .queue_at(vec![PROMOTE_EVENT, MOVE_TO, target, d], effect_tic)
                        {
                            tracing::warn!(%err, tic = t, "intent walk seed queue failed — queue dropped");
                            intent_queues.remove(&target);
                        } else {
                            let _ = event
                                .reducers()
                                .queue_at(q.fan_program(target), effect_tic);
                            tracing::info!(tic = t, interaction = %iname,
                                pawn = format!("{target:#010x}"),
                                dest = format!("{:?}", position_to_tile(d)),
                                pending = q.pending.len(),
                                "intent composed — walk queued, order parked behind it");
                        }
                        continue;
                    }
                    // ── SCHEDULE (lumberjack) ── a tic-costed intent validates now and
                    // queues its COMPLETION at +duration; the completion re-validates
                    // (the no-op law) and only then executes the effects below.
                    if !is_completion && params.duration > 0.0 {
                        let started_tic = master;
                        let fire_tic = tic_add(master, (params.duration as u16).max(4));
                        let mut prog = vec![
                            EXECUTE_INTERACTION,
                            interaction_ref,
                            INTENT_COMPLETION,
                            inputs.len() as u32,
                        ];
                        prog.extend_from_slice(inputs);
                        if let Err(err) = event.reducers().queue_at(prog, fire_tic) {
                            tracing::warn!(%err, tic = t, "intent completion queue failed — dropped");
                        } else {
                            let q = intent_queues.entry(target).or_insert_with(PawnQueue::new);
                            // An ADVANCED order keeps its display identity; a fresh
                            // in-place one mints its own (intent-queue-ui).
                            let entry_id = match q.advancing.take() {
                                Some((id, _)) if version == INTENT_ADVANCED => id,
                                other => {
                                    q.advancing = other; // not ours — put it back
                                    intent_entry_seq += 1;
                                    intent_entry_seq
                                }
                            };
                            q.running = Some(Running::Timed {
                                entry_id,
                                interaction_ref,
                                started_tic,
                                fire_tic,
                            });
                            let _ = event
                                .reducers()
                                .queue_at(q.fan_program(target), tic_add(master, 4));
                            tracing::info!(tic = t, interaction = %iname,
                                pawn = format!("{target:#010x}"),
                                duration = params.duration, fire_tic,
                                "intent scheduled — completion queued at +duration");
                        }
                        continue;
                    }
                    // An ADVANCED duration-0 order (a parked drink) executes below in
                    // this pass — its display entry leaves the strip now.
                    if version == INTENT_ADVANCED {
                        if let Some(q) = intent_queues.get_mut(&target) {
                            q.advancing = None;
                            let _ = event
                                .reducers()
                                .queue_at(q.fan_program(target), tic_add(master, 4));
                            if q.pending.is_empty() && q.running.is_none() {
                                intent_queues.remove(&target);
                            }
                        }
                    }
                    let mut program: Vec<u32> = Vec::new();
                    let mut satisfied_need: Option<String> = None;
                    let mut log_from = f64::NAN;
                    let mut log_to = f64::NAN;
                    // The satisfy effect: current satisfaction from the sub-table row (I3: a
                    // read-modify-write on a lazy value — benign while this relay is the
                    // single writer) through the PIECEWISE eval, then the SIGNED amount
                    // clamped to the EFFECTIVE domain and quantized ONCE (F4).
                    if let Some(satisfy) = &params.satisfy {
                        let need_ref = match &satisfy.need {
                            Operand::Input(i) => inputs[*i],
                            Operand::Name(n) => match bundle.gameplay_reference("need", n) {
                                Some(r) => r,
                                None => {
                                    reject("satisfy.need names an unknown need");
                                    continue;
                                }
                            },
                            Operand::Value(_) => {
                                reject("satisfy.need cannot be a number");
                                continue;
                            }
                        };
                        let amount = match &satisfy.amount {
                            Operand::Input(i) => f32::from_bits(inputs[*i]),
                            Operand::Value(v) => *v as f32,
                            Operand::Name(_) => {
                                reject("satisfy.amount cannot be a name");
                                continue;
                            }
                        };
                        if !amount.is_finite() {
                            reject("non-finite amount (I6: NaN/inf poison downstream math)");
                            continue;
                        }
                        let Some(np) = bundle.need_params_by_ref(need_ref) else {
                            reject("unknown need def");
                            continue;
                        };
                        let need_key = need_ref & 0xFFFF;
                        let Some(&(nrow, set_tic)) =
                            need_rows.iter().find(|(r, _)| r & 0xFFFF == need_key)
                        else {
                            reject("target carries no row for the need");
                            continue;
                        };
                        let (_, need_name) =
                            bundle.gameplay_lookup(need_ref).expect("resolved above");
                        let value = f64::from(resonantdust_codec::value::dequantize(
                            gameplay_row_data(nrow),
                            np.min as f32,
                            np.max as f32,
                        ));
                        let windows = resonantdust_content::needs_eval::rate_windows(
                            &bundle, &need_name, &trait_rows, &cond_rows, set_tic,
                        );
                        let now_sat = resonantdust_content::needs_eval::satisfaction_at(
                            value, set_tic, &np, effect_tic, &windows,
                        );
                        let (lo, hi) = resonantdust_content::needs_eval::need_bounds(
                            &bundle, &need_name, &np, &trait_rows, &cond_rows, effect_tic,
                        );
                        let new_sat = (now_sat + f64::from(amount)).clamp(lo, hi);
                        let q = resonantdust_codec::value::quantize(
                            new_sat as f32,
                            np.min as f32,
                            np.max as f32,
                        );
                        program.extend_from_slice(&[
                            PROMOTE,
                            SET_NEED,
                            target,
                            pack_gameplay_row(need_ref, q),
                        ]);
                        satisfied_need = Some(need_name);
                        log_from = now_sat;
                        log_to = new_sat;
                    }
                    // The move effect (input-rework F3/F6): queue the movement-chain SEED —
                    // the exact program the client used to send. PROMOTE_EVENT latches the
                    // intent fan (the speculation MoveIntent channel, I1); PROMOTE anchors
                    // the start; apply stamps the trip serial from THIS queued event's
                    // reference, so supersession works unchanged (I2).
                    if params.move_effect.is_some() {
                        let d = dest.expect("the location checks above guarantee a destination");
                        // pathfinding F5: an impathable or unreachable destination no-ops
                        // the order at SEED time — a water click is normal play, quietly
                        // refused here (menu graying is the recorded successor, I5).
                        if resonantdust_content::path_eval::find_path(
                            (px, py),
                            position_to_tile(d),
                            &cell_pathable,
                        )
                        .is_none()
                        {
                            reject("move_to no-op: destination impathable or unreachable (pathfinding F5)");
                            continue;
                        }
                        // chord-movement F4: intent only — the seed fans no position.
                        program.extend_from_slice(&[PROMOTE_EVENT, MOVE_TO, target, d]);
                    }
                    // The destroy effect (lumberjack F5): clear the validated CARRIER's
                    // cell through the cold overlay — the build-walls SET path, kind 0 =
                    // remove. When the carrier's BINDING authors a yield (logs-drop
                    // F1/F2), the SAME SET carries the yielded thing's kind_reference
                    // instead: destroy becomes a REPLACE — the tree's cell holds logs.
                    // Tile carriers refuse: nothing authors tile destruction.
                    let mut yielded: Option<String> = None;
                    if params.destroy.is_some() {
                        let (cpos, is_thing, cold_row, ckind) = carrier;
                        if !is_thing {
                            reject("destroy names a TILE carrier — only things fell (lumberjack F5)");
                            continue;
                        }
                        let yield_kind: u32 = bundle
                            .thing_interactions(ckind >> 4)
                            .iter()
                            .find(|b| b.name == iname)
                            .and_then(|b| b.yields.as_ref())
                            .and_then(|y| {
                                let k = bundle.thing_object_id(y);
                                if k.is_none() {
                                    // Load-validated, so only a registry drift reaches here.
                                    tracing::warn!(yield_name = %y,
                                        "yield names a thing the bundle cannot number — clearing instead");
                                }
                                yielded = k.map(|_| y.clone());
                                k.map(|id| u32::from(pack_kind_reference(id, 0)))
                            })
                            .unwrap_or(0);
                        let cell = u32::from((position_micro(cpos) >> 8) as u8);
                        program.extend_from_slice(&[
                            PROMOTE,
                            SET,
                            cold_row,
                            u32::from(TYPE_BIOME_THING),
                            cell,
                            yield_kind,
                            0,
                        ]);
                    }
                    // Grants carry remaining-at-write = the condition's authored duration
                    // (F3). The RE-STAMP duty is THIS composer's (F7/I12): any need a granted
                    // condition modifies that this program doesn't already SET gets its
                    // current value re-stamped beside the grant.
                    for g in &params.grants {
                        let Some(cref) = bundle.gameplay_reference("condition", g) else { continue };
                        let Some(cp) = bundle.condition_params(g) else { continue };
                        program.extend_from_slice(&[
                            PROMOTE,
                            GRANT_CONDITION,
                            target,
                            pack_gameplay_row(cref, cp.duration as u16),
                        ]);
                        for m in &cp.needs {
                            if satisfied_need.as_deref() == Some(m.need.as_str()) {
                                continue; // already re-stamped by the satisfy SET_NEED
                            }
                            let Some(mref) = bundle.gameplay_reference("need", &m.need) else {
                                continue;
                            };
                            let Some(mp) = bundle.need_params_by_ref(mref) else { continue };
                            let Some(&(mrow, mset)) =
                                need_rows.iter().find(|(r, _)| r & 0xFFFF == mref & 0xFFFF)
                            else {
                                continue; // the pawn doesn't carry this need — nothing to stamp
                            };
                            let mvalue = f64::from(resonantdust_codec::value::dequantize(
                                gameplay_row_data(mrow),
                                mp.min as f32,
                                mp.max as f32,
                            ));
                            let mwindows = resonantdust_content::needs_eval::rate_windows(
                                &bundle, &m.need, &trait_rows, &cond_rows, mset,
                            );
                            let msat = resonantdust_content::needs_eval::satisfaction_at(
                                mvalue, mset, &mp, effect_tic, &mwindows,
                            );
                            let mq = resonantdust_codec::value::quantize(
                                msat as f32,
                                mp.min as f32,
                                mp.max as f32,
                            );
                            program.extend_from_slice(&[
                                PROMOTE,
                                SET_NEED,
                                target,
                                pack_gameplay_row(mref, mq),
                            ]);
                        }
                    }
                    if let Err(err) = event.reducers().queue_at(program, effect_tic) {
                        tracing::warn!(%err, tic = t, "interaction effect queue failed — dropped");
                    } else {
                        // A BARE walk is an active event too (user, 2026-08-07: "move to
                        // interactions are not showing up in our intent queue despite
                        // executing"): a move-effect order registers as the queue's
                        // Running::Move — the strip's bottom circle — and the arrival
                        // poll completes it exactly like a composed walk. The fresh-order
                        // replace above already cleared any previous queue.
                        if params.move_effect.is_some() && !is_completion {
                            let d = dest.expect("the location checks above guarantee a destination");
                            intent_entry_seq += 1;
                            let q = intent_queues.entry(target).or_insert_with(PawnQueue::new);
                            q.running = Some(Running::Move {
                                entry_id: intent_entry_seq,
                                interaction_ref,
                                dest: d,
                            });
                            let _ = event
                                .reducers()
                                .queue_at(q.fan_program(target), effect_tic);
                        }
                        tracing::info!(tic = t, effect_tic, interaction = %iname,
                            target = format!("{target:#010x}"),
                            satisfied = ?satisfied_need, from = log_from, to = log_to,
                            moved = params.move_effect.is_some(),
                            destroyed = params.destroy.is_some(),
                            yielded = ?yielded,
                            dest = dest.map(|d| format!("{:?}", position_to_tile(d))),
                            grants = params.grants.len(), version,
                            "interaction executed");
                    }
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
                // QUEUE_STATE writes nothing, so its fan zone comes from the PAWN it
                // describes (intent-queue-ui F1) — without this the promoted event
                // completes zoneless and never reaches a subscriber.
                for inst in action::program(actions).flatten() {
                    if inst.action == QUEUE_STATE {
                        if let Some(p) = inst.operands.first() {
                            if let Some(r) =
                                pawn.db().entity_state().iter().find(|r| r.entity_reference == *p)
                            {
                                if !zones.contains(&r.macro_position_reference) {
                                    zones.push(r.macro_position_reference);
                                }
                            }
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
fn apply(
    actions: &[u32],
    scratch: &mut HashMap<u32, Payload>,
    promote: &mut HashSet<u32>,
    event_reference: u32,
    ctx: &ApplyCtx,
) {
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
                // MOVE_TO obj dest — ALWAYS the seed (ACTIONS.md §Movement): RESOLVE the
                // pawn's mid-chord position first (chord-movement F3 — an interrupting
                // order starts from where the pawn IS; the resolve reads the OLD chain's
                // dest from worker memory), stamp the trip-serial (this event's low 6
                // bits — the chain's identity, killing any OLDER chain at its next hop),
                // turn facing toward the path, and step NOTHING further. The seed fans NO
                // position (F4 — the start anchor retired; the client arms speculation
                // from its own belief). The worker chains hops from the CONTINUE pass.
                if let [obj, dest] = inst.operands {
                    let p = scratch.entry(*obj).or_default();
                    let resolved = resolve_walk_position_for(*obj, p, ctx);
                    if resolved != p.position_reference {
                        tracing::info!(
                            obj = format!("{obj:#010x}"),
                            from = ?resonantdust_codec::object::position_to_point(p.position_reference),
                            to = ?resonantdust_codec::object::position_to_point(resolved),
                            "mid-chord resolve — the new order starts where the pawn is"
                        );
                        p.position_reference = resolved;
                        p.base_tic = ctx.now;
                    }
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
                        let cur_tile = position_to_tile(p.position_reference);
                        let (tx, ty) = position_to_tile(*dest);
                        if cur_tile == (tx, ty) {
                            // On the dest tile: normalize onto the dest POINT exactly
                            // (a capped hop can land fractionally short of it).
                            p.position_reference = *dest;
                            p.base_tic = ctx.now;
                        } else {
                            // The shared route's FIRST CHORD (chord-movement F2/F3) —
                            // recomputed statelessly from the CURRENT point; the hop
                            // lands one STRIDE along it (F8: the re-anchor cadence as
                            // distance, chord-capped — I7).
                            use resonantdust_codec::object::{point_to_position, position_to_point};
                            match resonantdust_content::path_eval::find_chords(
                                cur_tile,
                                (tx, ty),
                                0,
                                ctx.pathable,
                            ) {
                                Some(chords) if !chords.is_empty() => {
                                    let (wx, wy) = chords[0];
                                    let (cxf, cyf) = position_to_point(p.position_reference);
                                    let (dxf, dyf) = (wx as f64 - cxf, wy as f64 - cyf);
                                    let len = dxf.hypot(dyf);
                                    if len > f64::EPSILON {
                                        let stride = hop_stride_tiles((ctx.tics_per_tile)(*obj));
                                        let f = (stride / len).min(1.0);
                                        p.position_reference =
                                            point_to_position(cxf + dxf * f, cyf + dyf * f);
                                        p.base_tic = ctx.now;
                                        let facing: u8 = if dxf.abs() >= dyf.abs() {
                                            if dxf > 0.0 { 1 } else { 3 }
                                        } else if dyf > 0.0 { 0 } else { 2 };
                                        p.data = pack_pawn_data(facing, pawn_trip_serial(p.data));
                                    }
                                }
                                _ => {
                                    // Blocked THIS hop (the world changed mid-trip): step
                                    // nothing. The chain keeps re-queuing and re-routing —
                                    // a cleared blockage resumes the trip by itself.
                                    tracing::debug!(
                                        obj = format!("{obj:#010x}"),
                                        "move_step blocked — no route this hop"
                                    );
                                }
                            }
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
    /// The row's write tic (chord-movement F3 — the resolve's time origin).
    tic: u16,
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
                tic: r.tic,
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
