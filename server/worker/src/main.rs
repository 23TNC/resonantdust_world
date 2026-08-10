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
    INIT_ZONE, INV_ADD, INV_REMOVE, MOVE_STEP, MOVE_TO, PLACE, PROMOTE, PROMOTE_EVENT,
    ACTIVATE_TRAIT, QUEUE_STATE, RESTAMP_NEED, SET, SET_NEED, SPAWN_REQUEST,
};
use resonantdust_codec::object::{
    cold_row_layer_id, cold_row_macro_position, cold_row_subtype, data_rotation, def_kind_id,
    def_type_id, kind_pos_ref_data, kind_pos_ref_kind_reference, kind_pos_ref_tile,
    pack_cold_row_reference, pack_kind_reference, pack_pawn_data, pack_position_reference,
    pawn_trip_serial, position_macro, position_micro, position_to_tile, tile_to_position,
    TYPE_BIOME_THING, TYPE_BIOME_TILE, TYPE_PAWN, TYPE_PLAYER,
};
use resonantdust_codec::speed;
use resonantdust_codec::refs::entity_ref_type_id;
use resonantdust_codec::status::{status_phase, EVENT_ASSIGNED, EVENT_COMPLETE};
use resonantdust_codec::tic::{tic_add, tic_after, tic_before};
use resonantdust_st_bindings::{data_shard, event_shard, index, pawn, player_pawn, thing, tile};
use resonantdust_uplink::acquire;
use data_shard::{write as _, EntityStateLogTableAccess as _};
use pawn::{grant_condition as _, inv_add as _, inv_remove as _, remove as _, set_need as _, spawn as _, write as _, EntityStateLogTableAccess as _};
// player-pawns P1 (I1): the routing lane's write half — the relay verbs + the hot write/base
// land on the player_pawn shard for TYPE_PLAYER targets.
use player_pawn::{
    grant_condition as _, set_need as _, write as _, EntityStateLogTableAccess as _,
    EntityStateTableAccess as _, NeedsTableAccess as _,
};
// interactions P3 / stat-model P3: the execute arm reads the pawn shard's COMPOSED rows
// (position + payload traits/conditions + the `needs` sub-table) and the tile shard's
// baseline ⊕ overlay for the F8 on-tile check.
use pawn::{EntityStateTableAccess as _, InventoryTableAccess as _, NeedsTableAccess as _, PayloadTableAccess as _};
use tile::{EntityStateTableAccess as _, OverlayTableAccess as _};
use event_shard::{complete as _, queue_at as _, EventLogTableAccess as _};
use index::{DefinitionsTableAccess as _, MasterClockTableAccess as _};
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
    /// The players' row-carrier shard (player-pawns P1) — `TYPE_PLAYER` targets.
    PlayerPawn,
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

/// The PART payload a spawn request implies (spawn-authority F3): for a kind whose corpus
/// declares ≥2 parts, one PART entry per slot with the def's variant nibble substituted
/// from the request's nibble vec — EXACTLY the words the webgl chat used to pack
/// client-side (opcode header `1<<16 | 2`, slot, def-with-variant). Single-part kinds
/// return empty (their variant rides the def itself — the wolf's coat).
fn mint_parts(def_sans_variant: u32, nibbles: &[u8; 8], part_count: usize) -> Vec<u32> {
    let mut words = Vec::new();
    if part_count >= 2 {
        for (slot, &v) in nibbles.iter().enumerate().take(part_count) {
            words.extend_from_slice(&[
                (1 << 16) | 2,
                slot as u32,
                def_sans_variant | u32::from(v),
            ]);
        }
    }
    words
}

/// The CREATE sidecars a def implies (stat-model F11): TRAIT payload entries + full-value
/// packed need rows, composed HERE from the corpus — the pawn module holds none.
/// The pawn's gameplay rows for eval — (trait rows, condition rows) — through THE
/// merged-traits accessor (trait-lights F5): the kind's CONSTANT binds derive beside
/// the payload's runtime rows, so the worker, npc and client evaluate the same traits
/// by construction. Kind comes from the pawn's own state row; no row → no rows.
fn pawn_gameplay_rows(
    bundle: &resonantdust_content::loader::Bundle,
    pawn: &pawn::DbConnection,
    entity: u32,
) -> (Vec<u64>, Vec<(u64, u16)>) {
    let kind = pawn
        .db()
        .entity_state()
        .iter()
        .find(|r| r.entity_reference == entity)
        .map(|r| def_kind_id(r.definition_reference))
        .unwrap_or(0);
    let (raw_traits, conditions) = pawn
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
    (bundle.object_trait_rows(kind, &raw_traits), conditions)
}

fn mint_sidecars(bundle: &resonantdust_content::loader::Bundle, def: u32) -> (Vec<u32>, Vec<u64>) {
    let kind = def_kind_id(def);
    let mut trait_rows: Vec<u64> = Vec::new();
    let mut trait_words = Vec::new();
    for b in bundle.thing_traits(kind) {
        // trait-rows-u32 F2: constancy is CATEGORY membership; the tier is the bind's
        // VARIANT on the def's reference. Constant binds join the eval set (the need
        // caps below read them) but never the stored payload.
        use resonantdust_codec::object as obj;
        let Some(cat) = bundle.trait_category(&b.name) else { continue };
        let Some(cat_name) = obj::gameplay_category(cat) else { continue };
        if let Some(r0) = bundle.gameplay_reference(cat_name, &b.name) {
            let r = (r0 & !0xF) | (u32::from(b.variant) & 0xF);
            let row = obj::pack_row(r, 0);
            trait_rows.push(row);
            let constant = cat == obj::GAMEPLAY_PAWN_TRAIT_CONSTANT
                || cat == obj::GAMEPLAY_PLAYER_TRAIT_CONSTANT;
            if !constant {
                trait_words
                    .extend_from_slice(&resonantdust_codec::payload::trait_entry(r, 0));
            }
        }
    }
    let mut need_rows = Vec::new();
    for nref in bundle.thing_needs(kind) {
        if let Some(np) = bundle.need_params_by_ref(nref) {
            // Needs mint FULL at the EFFECTIVE max (food-chain F2/I2): the kind's own
            // traits cap the ceiling (the leveled `corpus` trait — a bunny mints 1, a
            // wolf 2), quantized on the ONE authored encoding domain.
            let (_, name) = bundle
                .gameplay_lookup(nref)
                .unwrap_or_else(|| (String::new(), String::new()));
            let (_, eff_max) = resonantdust_content::needs_eval::need_bounds(
                bundle, &name, &np, &trait_rows, &[], 0,
            );
            let q = resonantdust_codec::value::quantize(
                eff_max as f32,
                np.min as f32,
                np.max as f32,
            );
            need_rows.push(resonantdust_codec::object::pack_row(nref, q));
        }
    }
    (trait_words, need_rows)
}

#[cfg(test)]
mod mint_tests {
    use super::mint_sidecars;
    use resonantdust_codec::object::{pack_definition_reference, pack_kind_reference};

    /// food-chain I2: the mint quantizes the EFFECTIVE max — the leveled corpus trait
    /// caps a bunny at 1 and a wolf at 2, on the ONE 0..2 encoding.
    #[test]
    fn needs_mint_at_the_effective_max() {
        let src = r##"
[[need]]
name = "corpus"
min = 0
max = 2

[[trait]]
name = "corpus"
needs = [ { need = "corpus", max = [1.0, 2.0] } ]

[[thing]]
name = "bunny"
type = "pawn"
kind = "bunny"
subType = ["animal"]
variant = ["0"]
needs = ["corpus"]
traits = [ { name = "corpus", level = 1 } ]
packed = [ { tint = "#b0a090" } ]

[[thing]]
name = "wolf"
type = "pawn"
kind = "wolf"
subType = ["animal"]
variant = ["0"]
needs = ["corpus"]
traits = [ { name = "corpus", level = 2 } ]
packed = [ { tint = "#5f6b3c" } ]
"##;
        let b = resonantdust_content::loader::load(&[("t.toml".into(), src.into())])
            .expect("fixture loads");
        let np = b.need_params("corpus").expect("corpus");
        let value_of = |kind_name: &str| -> f64 {
            let kind = b.thing_object_id(kind_name).expect(kind_name);
            let def = pack_definition_reference(0, pack_kind_reference(kind, 0));
            let (_, needs) = mint_sidecars(&b, def);
            let q = resonantdust_codec::object::row_data(needs[0]);
            f64::from(resonantdust_codec::value::dequantize(q, np.min as f32, np.max as f32))
        };
        assert!((value_of("bunny") - 1.0).abs() < 0.01, "the bunny mints 1");
        assert!((value_of("wolf") - 2.0).abs() < 0.01, "the wolf mints 2");
    }

    /// spawn-authority F3: the server's PART composition matches the words the webgl
    /// chat used to pack client-side, bit for bit — and single-part kinds compose none.
    #[test]
    fn mint_parts_matches_the_chat_words() {
        let def0 = 0x3005_0050u32; // a two-part human def, variant nibble 0
        let words = super::mint_parts(def0, &[5, 12, 0, 0, 0, 0, 0, 0], 2);
        assert_eq!(
            words,
            vec![(1 << 16) | 2, 0, def0 | 5, (1 << 16) | 2, 1, def0 | 12],
            "opcode header, slot, def-with-variant — the chat's exact shape"
        );
        assert!(super::mint_parts(def0, &[7, 0, 0, 0, 0, 0, 0, 0], 1).is_empty(),
            "a single-part kind mints bare (its variant rides the def)");
    }

    /// inventory F1/I2: the need counts FREE slots, so the SAME mint-at-effective-max
    /// law births a human with 6 free on the 0..16 encoding — no mint-empty rule.
    #[test]
    fn the_inventory_need_mints_all_slots_free() {
        let src = r##"
[[need]]
name = "inventory"
min = 0
max = 16

[[trait]]
name = "inventory"
needs = [ { need = "inventory", max = [6.0] } ]

[[thing]]
name = "human"
type = "pawn"
kind = "human"
subType = ["human"]
variant = ["0"]
needs = ["inventory"]
traits = [ { name = "inventory", level = 1 } ]
packed = [ { tint = "#c0b0a0" } ]
"##;
        let b = resonantdust_content::loader::load(&[("t.toml".into(), src.into())])
            .expect("fixture loads");
        let np = b.need_params("inventory").expect("inventory");
        let kind = b.thing_object_id("human").expect("human");
        let def = pack_definition_reference(0, pack_kind_reference(kind, 0));
        let (_, needs) = mint_sidecars(&b, def);
        let q = resonantdust_codec::object::row_data(needs[0]);
        let v = f64::from(resonantdust_codec::value::dequantize(q, np.min as f32, np.max as f32));
        assert!((v - 6.0).abs() < 0.01, "6 free slots at birth, got {v}");
    }

    /// trait-lights F5 (P2): a CONSTANT bind never mints a payload row — readers derive
    /// it — while its levels still cap the need mint (the trait is IN HAND at mint).
    #[test]
    fn mint_sidecars_skips_constant_binds() {
        let src = r##"
[[need]]
name = "corpus"
min = 0
max = 2

[[trait]]
name = "corpus"
needs = [ { need = "corpus", max = [1.0, 2.0] } ]

[[trait]]
name = "night_eyes"
emit_light = [ { color = "#88ff88", reach = 3 } ]

[[thing]]
name = "cat"
type = "pawn"
kind = "cat"
subType = ["animal"]
variant = ["0"]
needs = ["corpus"]
traits = [ { name = "corpus", level = 2 }, { name = "night_eyes", constant = true } ]
packed = [ { tint = "#303030" } ]
"##;
        let b = resonantdust_content::loader::load(&[("t.toml".into(), src.into())])
            .expect("fixture loads");
        let kind = b.thing_object_id("cat").expect("cat");
        let def = pack_definition_reference(0, pack_kind_reference(kind, 0));
        let (words, needs) = mint_sidecars(&b, def);
        // ONE stored trait entry (corpus — non-constant); night_eyes derives instead.
        let stored = resonantdust_codec::payload::payload_traits(&words);
        assert_eq!(stored.len(), 1, "only the non-constant bind mints: {stored:?}");
        let corpus_ref = b.gameplay_reference("trait", "corpus").expect("ref");
        assert_eq!(stored[0], resonantdust_codec::object::pack_row(corpus_ref, 2));
        // The constant bind still reaches eval through the accessor — and the light rides.
        let merged = b.object_trait_rows(kind, &stored);
        assert_eq!(merged.len(), 2, "constant + stored: {merged:?}");
        assert_eq!(b.object_lights(kind, &stored).len(), 1, "the cat's eyes glow");
        // The need still minted under the corpus cap (level 2 ⇒ the 0..2 encoding max).
        assert_eq!(needs.len(), 1);
    }
}

fn shard_of(entity: u32) -> Shard {
    match entity_ref_type_id(entity) {
        TYPE_BIOME_TILE => Shard::Tile,
        TYPE_BIOME_THING => Shard::Thing,
        TYPE_PAWN => Shard::Pawn,
        TYPE_PLAYER => Shard::PlayerPawn,
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

// The re-anchor cadence, the chord cap and the hop stride live in
// `resonantdust_content::move_eval` (shared-simulation P1) — THE walk, shared with every
// client so the worker's hops and an observer's belief cannot drift. The worker is a
// CALLER here, not the owner; it was the owner, privately, and that is the defect the
// shared-simulation stream exists to remove.
use resonantdust_content::move_eval::hop_stride_tiles;

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
    // `move_eval::position_at` (shared-simulation P1) — THE "where is this pawn now"
    // question, asked here by the worker and by every observer through the same code.
    // It carries the wrapping-tic guard (a future `base_tic` reads as elapsed 0, never
    // ancient — the needs-eval rule) and the off-centre clamp, which matters here because
    // the resolved point gets WRITTEN as the new order's start: an unclamped resolve
    // could park a pawn in the water the validated corridor skirted.
    let at = resonantdust_content::move_eval::position_at(
        position_to_point(p.position_reference),
        position_to_point(dest),
        p.base_tic,
        ctx.now,
        (ctx.tics_per_tile)(obj),
        ctx.pathable,
    );
    point_to_position(at.0, at.1)
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
    let player_pawn_db = env_or("PLAYER_PAWN_DB", "resonantdust-dev-player-pawn-0");
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
    // inventory F4: `definitions` rides along — the store effect writes the REGISTRY's
    // definition_reference for the picked-up thing (the corpus alone cannot number a
    // full def; only kinds — the npc/browser fetch /definitions for the same reason).
    let index_up = resonantdust_uplink::subbed_uplink!(index, "index", uri, index_db,
        vec![format!("SELECT * FROM master_clock WHERE realm = {realm}"),
             "SELECT * FROM definitions".to_string()]);
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
             "SELECT * FROM needs".to_string(), "SELECT * FROM inventory".to_string()]);
    // The player_pawn shard mirrors the pawn subscription shape (player-pawns P1): its live
    // rows gate the hot write (the ghost-row guard) and its composed rows feed the eval. The
    // population is one-per-player, so "all rows" is small by construction.
    let player_pawn_up = resonantdust_uplink::subbed_uplink!(player_pawn, "player_pawn", uri, player_pawn_db,
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
        ("player_pawn", player_pawn_up.get().await.is_ok()),
        ("tile", tile_up.get().await.is_ok()),
        ("thing", thing_up.get().await.is_ok()),
    ] {
        if !ok {
            tracing::warn!(%name, "not reachable at startup; will keep retrying");
        }
    }
    tracing::info!("resolving (uplinks lazy — a pass needing a dead upstream defers)");
    let (mut up_i, mut up_e, mut up_d, mut up_p, mut up_pp, mut up_t, mut up_h) =
        (true, true, true, true, true, true, true);

    // The per-pawn intent queues (lumberjack F1) — worker MEMORY, deliberately: the
    // in-flight order is a durable queued event; this map is only the tail behind it.
    let mut intent_queues: HashMap<u32, PawnQueue> = HashMap::new();
    // spawn-authority P5 (the teleport verdict, found live): a CROSS-ZONE order's event
    // lands in TWO work-groups (the orchestrator assigns by zone footprint, and the
    // pawn's zone and the dest's zone can split), so the EXECUTE_INTERACTION arm ran
    // once PER GROUP — two walk chains one tic apart, leapfrogging authoritative rows,
    // the user-visible teleport. Each (event, instruction) executes ONCE per session;
    // a worker bounce forgets the set and the in-band-behind replay posture is unchanged.
    let mut executed_interactions: HashSet<(u32, u32)> = HashSet::new();
    // survival F4/I2: the crossing scheduler's pending slots — (pawn, need_ref) → the
    // queued RESTAMP's fire tic. Worker MEMORY on purpose: the ensure pass below
    // re-derives schedules from live rows every tic, so a bounce loses nothing but a
    // few duplicate (harmless, re-validating) re-stamps.
    let mut crossing_slots: HashMap<(u32, u32), u16> = HashMap::new();
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
        let Some(player_pawn) = acquire(&player_pawn_up, &mut up_pp, "player_pawn").await else { continue };
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
            let (trait_rows, cond_rows) = pawn_gameplay_rows(&bundle, &pawn, entity);
            let need_rows: Vec<(u64, u16)> = pawn
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
                match event.reducers().queue_at(prog, tic_add(master, 4), 0) {
                    Ok(()) => tracing::info!(master, pawn = format!("{p:#010x}"),
                        pending = q.pending.len(), "intent advanced — walk landed, next order queued"),
                    Err(err) => tracing::warn!(%err, pawn = format!("{p:#010x}"),
                        "intent advance queue failed — pending tail dropped"),
                }
            }
            let _ = event.reducers().queue_at(q.fan_program(p), tic_add(master, 4), 0);
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
                // food-chain I11: a REMOVED pawn's in-flight chain must not resurrect it from
                // log history — a pawn target with no LIVE row drops out of the compose and its
                // events complete as no-ops. Minted pawns are unaffected: `spawn` writes their
                // first live row before any event names them (CREATE operands never carry the id).
                .filter(|e| {
                    // player-pawns P1: the same ghost-row guard on the player_pawn shard —
                    // a target with no live row composes nothing (its events no-op).
                    if shard_of(*e) == Shard::PlayerPawn {
                        let live = player_pawn.db().entity_state().iter().any(|r| r.entity_reference == *e);
                        if !live {
                            tracing::info!(target = format!("{e:#010x}"), tic = t, "no live player-pawn row — dropping its events (player-pawns P1)");
                        }
                        return live;
                    }
                    if shard_of(*e) != Shard::Pawn {
                        return true;
                    }
                    let live = pawn.db().entity_state().iter().any(|r| r.entity_reference == *e);
                    if !live {
                        tracing::info!(target = format!("{e:#010x}"), tic = t, "no live pawn row — dropping the dead's events (food-chain I11)");
                    }
                    live
                })
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
                if base_row(&data, &pawn, &player_pawn, e, t).map(|b| b.dirty).unwrap_or(false) {
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
                let base = base_row(&data, &pawn, &player_pawn, e, t)
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
                // food-chain I11, the WRITE half: `apply` or_defaults entries for
                // event targets the hot filter dropped (the dead) — writing those
                // would re-insert a DEFAULT (def-0 ghost) row. Only seeded targets
                // may write.
                .filter(|(e, _)| hot_targets.contains(e))
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
            // player-pawns P1: the row-carriers' shard takes its own absolute finals — the
            // pawn arm's posture verbatim (live/seeded targets only; the ghost guard above).
            let player_pawn_w: Vec<player_pawn::TargetState> = scratch
                .iter()
                .filter(|(&e, _)| shard_of(e) == Shard::PlayerPawn)
                .filter(|(e, _)| hot_targets.contains(e))
                .map(|(&e, p)| player_pawn::TargetState {
                    entity_reference: e,
                    definition_reference: p.definition_reference,
                    macro_position_reference: position_macro(p.position_reference),
                    micro_position_reference: position_micro(p.position_reference),
                    data: p.data,
                    promote: promote.contains(&e),
                })
                .collect();
            if !player_pawn_w.is_empty() {
                if let Err(err) = player_pawn.reducers().write(self_ref, t, player_pawn_w) {
                    tracing::warn!(%err, tic = t, shard = "player_pawn", "write failed — will retry next pass");
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
            // npc-host I11: the issuer per event — forwarded into the spawn ledger (ownership).
            let issuer_of: HashMap<u32, u32> = event
                .db()
                .event_log()
                .iter()
                .map(|e| (e.event_reference, e.issuer_player_id))
                .collect();
            let mut create_zones: HashMap<u32, Vec<u16>> = HashMap::new();
            let mut spawn_failed = false;
            // spawn-authority I4: is a def (variant nibble 0) REGISTERED? Matches any
            // registry row differing only in the variant nibble — captured HERE because
            // the ledger counter below shadows the `index` connection's name.
            let def_registered = |def0: u32| -> bool {
                index.db().definitions().iter().any(|d| d.id & !0xF == def0)
            };
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
                                        0, // a bare CREATE seeds no facing (spawn-authority I9)
                                        pending_promote,
                                        issuer_of.get(event_reference).copied().unwrap_or(0),
                                    ) {
                                        tracing::warn!(%err, tic = t, "spawn failed — will retry next pass");
                                        spawn_failed = true;
                                    }
                                    create_zones.entry(*event_reference).or_default().push(position_macro(*pos));
                                }
                                index += 1;
                            }
                        }
                        // ── SPAWN_REQUEST (xy rot_def variants) ── THE AUTHORITY DOOR
                        // (spawn-authority F1/F2/F3/F6): validate, then ROUTE by the TYPE
                        // nibble — pawns mint HERE through the SAME ledger (keyed by the
                        // REQUEST event, so replays dedupe — I5 dissolves); things/tiles
                        // queue the validated SET. REFUSE, never nudge (F2).
                        SPAWN_REQUEST => {
                            if let [xy, rot_def, variants] = inst.operands {
                                let (x, y, rot, def0, nib) =
                                    action::unpack_spawn_request(*xy, *rot_def, *variants);
                                let refuse = |why: &str| {
                                    tracing::warn!(
                                        event = format!("{event_reference:#010x}"),
                                        x, y, rot, def = format!("{def0:#010x}"),
                                        why, tic = t,
                                        "spawn request refused (spawn-authority F2)"
                                    );
                                };
                                // The world addresses 12 bits per axis today.
                                if x >= 4096 || y >= 4096 {
                                    refuse("position out of world");
                                } else if rot > 3 {
                                    refuse("rotation out of range (0..3)");
                                } else if !def_registered(def0) {
                                    refuse("def not in the registry (I4)");
                                } else {
                                    let type_id = def_type_id(def0);
                                    let kind = def_kind_id(def0);
                                    let (xi, yi) = (i32::from(x), i32::from(y));
                                    let pos = tile_to_position(xi, yi);
                                    let zone = position_macro(pos);
                                    let cell = (position_micro(pos) >> 8) as u8;
                                    match type_id {
                                        TYPE_PAWN => 'pawn_req: {
                                            if !cell_pathable(xi, yi) {
                                                refuse("cell impathable (the lake-mint law)");
                                                break 'pawn_req;
                                            }
                                            // The corpus part declarations are the tail's
                                            // CONTRACT (F3): stray nonzero nibbles refuse.
                                            let part_count = bundle
                                                .visual_for_object(kind)
                                                .map(|v| v.parts.len())
                                                .unwrap_or(1)
                                                .max(1);
                                            if nib.iter().skip(part_count).any(|&v| v != 0) {
                                                refuse("variant nibbles beyond the declared parts (F3)");
                                                break 'pawn_req;
                                            }
                                            // A single-part kind wears nibble 0 on its OWN def.
                                            let def = if part_count == 1 {
                                                def0 | u32::from(nib[0])
                                            } else {
                                                def0
                                            };
                                            let mut words = mint_parts(def0, &nib, part_count);
                                            let (trait_words, need_rows) =
                                                mint_sidecars(&bundle, def);
                                            words.extend_from_slice(&trait_words);
                                            let data = resonantdust_codec::object::pack_pawn_data(rot, 0);
                                            if let Err(err) = pawn.reducers().spawn(
                                                self_ref, t, *event_reference, index, def, pos,
                                                words, need_rows, data,
                                                true, // a spawn you can't see is useless — always promote
                                                issuer_of.get(event_reference).copied().unwrap_or(0),
                                            ) {
                                                tracing::warn!(%err, tic = t, "requested spawn failed — will retry next pass");
                                                spawn_failed = true;
                                            } else {
                                                tracing::info!(tic = t, x, y, rot,
                                                    def = format!("{def:#010x}"),
                                                    parts = part_count,
                                                    "spawn request minted (spawn-authority)");
                                            }
                                            create_zones.entry(*event_reference).or_default().push(zone);
                                        }
                                        TYPE_BIOME_THING | TYPE_BIOME_TILE => 'cold_req: {
                                            if type_id == TYPE_BIOME_THING
                                                && thing_kind_at(zone, cell).is_some()
                                            {
                                                refuse("cell occupied on the thing layer (F6)");
                                                break 'cold_req;
                                            }
                                            let kr = u32::from(pack_kind_reference(kind, nib[0]));
                                            let cold_row = pack_cold_row_reference(zone, 0, 0);
                                            let prog = vec![
                                                PROMOTE, SET, cold_row, u32::from(type_id),
                                                u32::from(cell), kr, u32::from(rot),
                                            ];
                                            if let Err(err) =
                                                event.reducers().queue_at(prog, tic_add(master, 4), 0)
                                            {
                                                tracing::warn!(%err, tic = t, "requested cold spawn queue failed — dropped");
                                            } else {
                                                tracing::info!(tic = t, x, y,
                                                    kind_reference = format!("{kr:#06x}"),
                                                    layer = type_id,
                                                    "spawn request queued a cold SET (spawn-authority F6)");
                                            }
                                        }
                                        _ => refuse("no spawn arm for this def type"),
                                    }
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
            // survival D1: the crossing re-stamps this pass computed — the sweep below
            // consumes them exactly like SET_NEED writes (the death lane).
            let mut restamps: Vec<(u32, u64)> = Vec::new();
            for (_event_reference, actions) in &events {
                for inst in action::program(actions) {
                    let Ok(inst) = inst else { break };
                    let r = match (inst.action, inst.operands) {
                        // survival F4/D1: the crossing re-stamp — evaluate the need's
                        // CURRENT lazy value (never a queued prediction) and write it.
                        (RESTAMP_NEED, [obj, nref]) => {
                            crossing_slots.remove(&(*obj, *nref));
                            let (trait_rows, cond_rows) = pawn_gameplay_rows(&bundle, &pawn, *obj);
                            let need_rows: Vec<(u64, u16)> = pawn
                                .db()
                                .needs()
                                .iter()
                                .filter(|r| r.entity_reference == *obj)
                                .map(|r| (r.need, r.set_tic))
                                .collect();
                            let Some((_, name)) = bundle.gameplay_lookup(*nref) else { continue };
                            let Some(np) = bundle.need_params_by_ref(*nref) else { continue };
                            let Some(sat) = resonantdust_content::needs_eval::need_satisfaction(
                                &bundle, &name, &trait_rows, &need_rows, &cond_rows, t,
                            ) else {
                                continue; // no row (a dead/absent pawn) — nothing to stamp
                            };
                            let row = resonantdust_codec::object::pack_row(
                                *nref,
                                resonantdust_codec::value::quantize(
                                    sat as f32, np.min as f32, np.max as f32,
                                ),
                            );
                            restamps.push((*obj, row));
                            tracing::info!(tic = t, pawn = format!("{obj:#010x}"), need = %name,
                                value = sat, "crossing re-stamp (survival F4)");
                            pawn.reducers().set_need(self_ref, t, *obj, row)
                        }
                        // stat-model: both verbs carry ONE packed gameplay row beside the obj.
                        // player-pawns P1: the relay lands on the target's OWN shard by its
                        // type nibble — a TYPE_PLAYER row-carrier composes on player_pawn.
                        // trait-rows-u32 F3: ACTIVATE_TRAIT — validate, then queue the
                        // def's authored condition GRANTS. The LOAD door already bounds
                        // active binds at 3; `blocked_by` is the run gate (sprint's
                        // cooldown); an unbound/inactive-category trait refuses by name.
                        (ACTIVATE_TRAIT, [obj, tref]) => {
                            use resonantdust_codec::object as codec_obj;
                            let reject = |why: &str| {
                                tracing::warn!(tic = t, target = format!("{obj:#010x}"),
                                    tref = format!("{tref:#010x}"), why, "ACTIVATE_TRAIT refused");
                            };
                            let kind = match shard_of(*obj) {
                                Shard::PlayerPawn => player_pawn
                                    .db()
                                    .entity_state()
                                    .iter()
                                    .find(|r| r.entity_reference == *obj)
                                    .map(|r| def_kind_id(r.definition_reference)),
                                _ => pawn
                                    .db()
                                    .entity_state()
                                    .iter()
                                    .find(|r| r.entity_reference == *obj)
                                    .map(|r| def_kind_id(r.definition_reference)),
                            };
                            let Some(kind) = kind else {
                                reject("no live carrier row");
                                continue;
                            };
                            // The bind check runs on the CARRIER's def: things bind pawn
                            // traits; brains (player-pawn defs) bind player traits.
                            let binds = if shard_of(*obj) == Shard::PlayerPawn {
                                let bid = bundle
                                    .gameplay_lookup(*tref)
                                    .and_then(|(_, name)| {
                                        // the player-pawn's def is a BRAIN def — its binds
                                        // resolve through the brain lane by def kind.
                                        let _ = name;
                                        Some(bundle.brain_player_traits(kind))
                                    });
                                bid.unwrap_or_default()
                            } else {
                                bundle.thing_traits(kind)
                            };
                            let bound = binds.iter().any(|b| {
                                bundle
                                    .trait_category(&b.name)
                                    .and_then(codec_obj::gameplay_category)
                                    .and_then(|c| bundle.gameplay_reference(c, &b.name))
                                    .map(|r0| (r0 & !0xF) | u32::from(b.variant))
                                    == Some(*tref)
                            });
                            if !bound {
                                reject("the carrier does not bind this trait");
                                continue;
                            }
                            let Some(tp) = bundle.trait_params_by_ref(*tref) else {
                                reject("unknown trait def");
                                continue;
                            };
                            let cat = codec_obj::def_subtype_id(*tref);
                            if cat != codec_obj::GAMEPLAY_PAWN_TRAIT_ACTIVE
                                && cat != codec_obj::GAMEPLAY_PLAYER_TRAIT_ACTIVE
                            {
                                reject("not an ACTIVE trait category");
                                continue;
                            }
                            if tp.activate.is_empty() {
                                reject("the trait authors no activation grants");
                                continue;
                            }
                            // The availability predicate: every blocked_by must be inactive.
                            let (trait_rows, cond_rows) = if shard_of(*obj) == Shard::PlayerPawn {
                                (Vec::new(), Vec::new())
                            } else {
                                pawn_gameplay_rows(&bundle, &pawn, *obj)
                            };
                            let need_rows: Vec<(u64, u16)> = if shard_of(*obj) == Shard::PlayerPawn {
                                player_pawn
                                    .db()
                                    .needs()
                                    .iter()
                                    .filter(|r| r.entity_reference == *obj)
                                    .map(|r| (r.need, r.set_tic))
                                    .collect()
                            } else {
                                pawn.db()
                                    .needs()
                                    .iter()
                                    .filter(|r| r.entity_reference == *obj)
                                    .map(|r| (r.need, r.set_tic))
                                    .collect()
                            };
                            let active = resonantdust_content::needs_eval::active_conditions(
                                &bundle, &trait_rows, &need_rows, &cond_rows, t,
                            );
                            let blocked = tp.blocked_by.iter().any(|name| {
                                bundle
                                    .gameplay_reference("condition", name)
                                    .is_some_and(|cref| active.iter().any(|c| c.condition_id == cref))
                            });
                            if blocked {
                                reject("blocked by an active condition (the cooldown gate)");
                                continue;
                            }
                            let mut program = Vec::new();
                            for (cond, duration) in &tp.activate {
                                let Some(cref) = bundle.gameplay_reference("condition", cond)
                                else {
                                    continue;
                                };
                                program.extend_from_slice(&[
                                    PROMOTE,
                                    GRANT_CONDITION,
                                    *obj,
                                    cref,
                                    u32::from(*duration),
                                ]);
                            }
                            let fire = tic_add(master, 4);
                            if let Err(err) = event.reducers().queue_at(program, fire, 0) {
                                tracing::warn!(%err, tic = t, "activation grants queue failed");
                            } else {
                                tracing::info!(tic = t, target = format!("{obj:#010x}"),
                                    tref = format!("{tref:#010x}"), grants = tp.activate.len(),
                                    "trait ACTIVATED (trait-rows-u32 F3)");
                            }
                            continue;
                        }
                        (SET_NEED, [obj, nref, data]) if shard_of(*obj) == Shard::PlayerPawn => {
                            player_pawn.reducers().set_need(
                                self_ref, t, *obj,
                                resonantdust_codec::object::pack_row(*nref, *data as u16),
                            )
                        }
                        (SET_NEED, [obj, nref, data]) => pawn.reducers().set_need(
                            self_ref, t, *obj,
                            resonantdust_codec::object::pack_row(*nref, *data as u16),
                        ),
                        (GRANT_CONDITION, [obj, cref, data]) if shard_of(*obj) == Shard::PlayerPawn => {
                            player_pawn.reducers().grant_condition(
                                self_ref, t, *obj,
                                resonantdust_codec::object::pack_row(*cref, *data as u16),
                            )
                        }
                        (GRANT_CONDITION, [obj, cref, data]) => pawn.reducers().grant_condition(
                            self_ref, t, *obj,
                            resonantdust_codec::object::pack_row(*cref, *data as u16),
                        ),
                        // inventory F3: the item verbs relay the same way — the module owns
                        // the slot scan; the free-count SET_NEED rides the same program.
                        (INV_ADD, [obj, item]) => pawn.reducers().inv_add(self_ref, t, *obj, *item),
                        (INV_REMOVE, [obj, slot]) => {
                            pawn.reducers().inv_remove(self_ref, t, *obj, (*slot & 0xFF) as u8)
                        }
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

            // ── THE NEED-WRITE TRIGGER (food-chain F5, the user's law) ── "when a need
            // is modified, is when that affordance is fired": every SET_NEED sweeps the
            // TARGET's carried interactions for affordances CHECKING the written need,
            // evaluates them against the pawn's rows WITH THE FRESH WRITE SUBSTITUTED
            // (the mirror may lag the reducer), and queues each passer as a FRESH
            // system order through the same event door everything rides — the fire-time
            // re-validation keeps the barrier window honest (a heal saves the pawn).
            let mut swept: HashSet<(u32, u32)> = HashSet::new();
            // survival D1: the sweep consumes SET_NEED writes AND this pass's crossing
            // re-stamps through one list — a re-stamp at ≤ 0 fires death exactly like
            // any other corpus write.
            let mut sweep_writes: Vec<(u32, u64)> = restamps.clone();
            for (_er, actions) in &events {
                for inst in action::program(actions) {
                    let Ok(inst) = inst else { break };
                    if let (SET_NEED, [obj, nref, data]) = (inst.action, inst.operands) {
                        sweep_writes
                            .push((*obj, resonantdust_codec::object::pack_row(*nref, *data as u16)));
                    }
                }
            }
            for &(obj_v, row_v) in &sweep_writes {
                {
                    let (obj, row) = (&obj_v, &row_v);
                    let nref = resonantdust_codec::object::row_reference(*row);
                    if !swept.insert((*obj, nref)) {
                        continue; // one sweep per (pawn, need) per tic
                    }
                    let Some((_, need_name)) = bundle.gameplay_lookup(nref) else { continue };
                    let Some(prow) =
                        pawn.db().entity_state().iter().find(|r| r.entity_reference == *obj)
                    else {
                        continue;
                    };
                    let kind = def_kind_id(prow.definition_reference);
                    let (trait_rows, cond_rows) = pawn_gameplay_rows(&bundle, &pawn, *obj);
                    // The mirrored needs, with THIS write upserted.
                    let mut need_rows: Vec<(u64, u16)> = pawn
                        .db()
                        .needs()
                        .iter()
                        .filter(|r| r.entity_reference == *obj)
                        .map(|r| (r.need, r.set_tic))
                        .collect();
                    let key = resonantdust_codec::object::row_reference(*row);
                    need_rows
                        .retain(|(r, _)| resonantdust_codec::object::row_reference(*r) != key);
                    need_rows.push((*row, t));
                    let active = resonantdust_content::needs_eval::active_conditions(
                        &bundle, &trait_rows, &need_rows, &cond_rows, t,
                    );
                    for bind in bundle.thing_interactions(kind) {
                        let Some(ip) = bundle.interaction_params(&bind.name) else { continue };
                        let keyed = ip.affordances.iter().any(|a| {
                            bundle
                                .affordance_params(a)
                                .and_then(|p| p.trigger_need().map(str::to_owned))
                                .as_deref()
                                == Some(need_name.as_str())
                        });
                        if !keyed {
                            continue;
                        }
                        if !resonantdust_content::stat_eval::interaction_available(
                            &bundle, &bind.name, &trait_rows, &need_rows, &cond_rows, &active, t,
                        ) {
                            continue;
                        }
                        let Some(iref) = bundle.gameplay_reference("interaction", &bind.name)
                        else {
                            continue;
                        };
                        // Bind the input signature exactly as the pie menu would.
                        let mut inputs = Vec::with_capacity(ip.inputs.len());
                        let mut bindable = true;
                        for name in &ip.inputs {
                            match name.as_str() {
                                "pawn" => inputs.push(*obj),
                                "destination" => inputs.push(pack_position_reference(
                                    prow.macro_position_reference,
                                    prow.micro_position_reference,
                                )),
                                "amount" => inputs.push((bind.magnitude as f32).to_bits()),
                                other => {
                                    tracing::warn!(interaction = %bind.name, input = other,
                                        "need-trigger: unbindable input — not fired");
                                    bindable = false;
                                }
                            }
                        }
                        if !bindable {
                            continue;
                        }
                        let mut program =
                            vec![EXECUTE_INTERACTION, iref, INTENT_FRESH, inputs.len() as u32];
                        program.extend_from_slice(&inputs);
                        match event.reducers().queue_at(program, tic_add(master, 4), 0) {
                            Ok(()) => tracing::info!(tic = t, interaction = %bind.name,
                                need = %need_name, pawn = format!("{obj:#010x}"),
                                "need-write trigger fired (food-chain F5)"),
                            Err(err) => tracing::warn!(%err, tic = t, interaction = %bind.name,
                                "need-write trigger queue failed"),
                        }
                    }
                }
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
                    if let Err(err) = event.reducers().queue_at(program, next_tic, 0) {
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
                            if let Err(err) = event.reducers().queue_at(program, next_tic, 0) {
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
            // ── THE CROSSING SCHEDULER (survival F4/I2/I4/I8) ── every pass, ENSURE
            // each live pawn's drain-target needs carry one pending RESTAMP at their
            // predicted floor crossing. Ensure-not-event-driven on purpose: a fresh
            // mint's needs decay with NO write ever arriving, and a bounced worker's
            // lost slots re-derive from the live rows (I4's durability for free). An
            // EARLIER prediction supersedes (queue + overwrite the slot; the stale
            // event re-validates harmlessly); a LATER one waits for the pending fire.
            {
                let drain_targets: Vec<(String, u32)> = bundle
                    .drain_target_needs()
                    .into_iter()
                    .filter_map(|n| bundle.gameplay_reference("need", &n).map(|r| (n, r)))
                    .collect();
                if !drain_targets.is_empty() {
                    let live: HashSet<u32> =
                        pawn.db().entity_state().iter().map(|r| r.entity_reference).collect();
                    crossing_slots.retain(|(p, _), fire| {
                        live.contains(p) && fire.wrapping_sub(t) <= u16::MAX / 2
                    });
                    for &p in &live {
                        let (trait_rows, cond_rows) = pawn_gameplay_rows(&bundle, &pawn, p);
                        let need_rows: Vec<(u64, u16)> = pawn
                            .db()
                            .needs()
                            .iter()
                            .filter(|r| r.entity_reference == p)
                            .map(|r| (r.need, r.set_tic))
                            .collect();
                        for (name, nref) in &drain_targets {
                            let Some(np) = bundle.need_params_by_ref(*nref) else { continue };
                            let Some(cross) = resonantdust_content::needs_eval::floor_crossing_tic(
                                &bundle, name, np.min, &trait_rows, &need_rows, &cond_rows,
                            ) else {
                                continue;
                            };
                            // The horizon clamp (I8): a crossing beyond the safe window
                            // re-stamps at the horizon and chains from the fresh row.
                            let ahead = cross.wrapping_sub(t);
                            let fire = if ahead > 20_000 { tic_add(t, 20_000) } else { cross };
                            let fire = if fire.wrapping_sub(t) < 4 { tic_add(master, 4) } else { fire };
                            let key = (p, *nref);
                            let earlier = crossing_slots
                                .get(&key)
                                .is_none_or(|pending| fire.wrapping_sub(t) < pending.wrapping_sub(t));
                            if earlier {
                                if event
                                    .reducers()
                                    .queue_at(vec![RESTAMP_NEED, p, *nref], fire, 0)
                                    .is_ok()
                                {
                                    crossing_slots.insert(key, fire);
                                    tracing::info!(tic = t, pawn = format!("{p:#010x}"),
                                        need = %name, fire, "crossing re-stamp scheduled (survival F4)");
                                }
                            }
                        }
                    }
                }
            }

            // Pass-local inventory adds (inventory I2): two SAME-TIC pick_up completions
            // read the same table snapshot — this count keeps the second from overfilling.
            let mut inv_adds_this_pass: HashMap<u32, usize> = HashMap::new();
            // Bound the dedup set (a u32-pair per interaction ever executed — clear far
            // before it matters; a cleared dup window is narrower than a worker bounce).
            if executed_interactions.len() > 100_000 {
                executed_interactions.clear();
            }
            for (event_reference, actions) in &events {
                for (inst_index, inst) in action::program(actions).enumerate() {
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
                                .queue_at(q.fan_program(*pawn_ref), tic_add(master, 4), 0);
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
                                        .queue_at(q.fan_program(*pawn_ref), tic_add(master, 4), 0);
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
                                0);
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
                    // The cross-zone double-assignment guard (see `executed_interactions`).
                    if !executed_interactions.insert((*event_reference, inst_index as u32)) {
                        tracing::warn!(event = format!("{event_reference:#010x}"), tic = t,
                            "duplicate work-group assignment — interaction already executed, SKIPPED (teleport verdict)");
                        continue;
                    }
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
                    use resonantdust_codec::object::{pack_row, row_data};
                    // The acting PAWN: the reserved `pawn` input FIRST (input-rework F5) —
                    // an effect's target can name ANOTHER pawn now (attack F2: satisfy
                    // hits `@target`, the VICTIM), so binding the actor from the effect
                    // evaluated the AFFORDANCES on the prey (the bunny failed can_attack —
                    // seen live). The effect-target fallback covers a signature without a
                    // `pawn` input, none of which exist today.
                    let target_op = params
                        .satisfy
                        .as_ref()
                        .map(|s| &s.target)
                        .or(params.move_effect.as_ref().map(|m| &m.target));
                    let target = match params.inputs.iter().position(|n| n == "pawn") {
                        Some(i) => inputs[i],
                        None => match target_op {
                            Some(&Operand::Input(ti)) => inputs[ti],
                            _ => {
                                reject("no `pawn` input or effect target names the acting pawn");
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
                            .queue_at(PawnQueue::new().fan_program(target), tic_add(master, 4), 0);
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
                                    match event.reducers().queue_at(prog, tic_add(master, 4), 0) {
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
                                    .queue_at(q.fan_program(target), tic_add(master, 4), 0);
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
                    // attack F2: a `target` input names a PAWN carrier — the first hot
                    // carrier that is not the actor itself. Resolve its LIVE row NOW
                    // (and again at every completion): the position feeds adjacency +
                    // the walk's dest, the kind feeds the offer. Dead prey = a logged
                    // no-op — a raced double-kill never swings at a ghost.
                    let mut victim: Option<(u32, u16)> = None;
                    if let Some(i) = params.inputs.iter().position(|n| n == "target") {
                        let vref = inputs[i];
                        let Some(vrow) =
                            pawn.db().entity_state().iter().find(|r| r.entity_reference == vref)
                        else {
                            reject("the target pawn is not alive (attack F2)");
                            continue;
                        };
                        victim = Some((
                            pack_position_reference(
                                vrow.macro_position_reference,
                                vrow.micro_position_reference,
                            ),
                            resonantdust_codec::object::def_kind_reference(
                                vrow.definition_reference,
                            ),
                        ));
                    }
                    let dest = dest.or(victim.map(|(p, _)| p));
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
                        // food-chain I9: `self` — the carrier is the pawn; no cells to scan.
                        "self" => Vec::new(),
                        // inventory F5: `slot` — the carrier is an inventory slot of the
                        // acting pawn; resolved below, no cells to scan.
                        "slot" => Vec::new(),
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
                    // Drop's slot context (inventory F5/I5): the validated (slot, item def)
                    // pair — set only under `location = "slot"`, read by spawn "carried".
                    let mut slot_ctx: Option<(u8, u32)> = None;
                    if params.location == "self" {
                        // food-chain I9: the ONE hot-carrier rule — the carrier IS the
                        // target pawn (death rides its own kind); no cold probe.
                        let kr = resonantdust_codec::object::def_kind_reference(
                            prow.definition_reference,
                        );
                        if bundle.thing_interactions(kr >> 4).iter().any(|b| b.name == iname) {
                            carrier = Some((pawn_pos, true, 0, kr));
                        }
                    }
                    if let Some((vpos, vkr)) = victim {
                        // attack F2: the VICTIM's kind must OFFER this interaction (the
                        // bunny carries `attack` the way water carries drink); the
                        // carrier tuple hands its kind to the binding lookup below
                        // (magnitude) exactly like a cold carrier's.
                        if bundle.thing_interactions(vkr >> 4).iter().any(|b| b.name == iname) {
                            carrier = Some((vpos, true, 0, vkr));
                        }
                    }
                    if params.location == "slot" {
                        // inventory F5: the carrier is an inventory SLOT of the acting pawn.
                        // The clicked slot + the item the clicker SAW ride the inputs; a
                        // mismatch with the CURRENT row is a logged no-op (I5) — never
                        // "drop whatever is there now". The pawn's OWN kind must bind the
                        // interaction (the death `self` rule); the carrier KIND is the
                        // ITEM's, so spawn "carried" reads it off the carrier tuple.
                        let slot_in = params.inputs.iter().position(|n| n == "slot").map(|i| inputs[i]);
                        let item_in = params.inputs.iter().position(|n| n == "item").map(|i| inputs[i]);
                        let (Some(slot_in), Some(item_in)) = (slot_in, item_in) else {
                            reject("location `slot` needs slot + item inputs (inventory F5)");
                            continue;
                        };
                        let row = pawn
                            .db()
                            .inventory()
                            .iter()
                            .find(|r| r.entity_reference == target && u32::from(r.slot) == slot_in);
                        let Some(row) = row else {
                            reject("slot re-validation: the slot is empty (inventory I5)");
                            continue;
                        };
                        if row.item != item_in {
                            reject("slot re-validation: the slot no longer holds that item (inventory I5)");
                            continue;
                        }
                        let pawn_kr = resonantdust_codec::object::def_kind_reference(
                            prow.definition_reference,
                        );
                        if bundle.thing_interactions(pawn_kr >> 4).iter().any(|b| b.name == iname) {
                            let item_kr =
                                resonantdust_codec::object::def_kind_reference(row.item);
                            slot_ctx = Some((row.slot, row.item));
                            carrier = Some((pawn_pos, true, 0, item_kr));
                        }
                    }
                    for &c in &candidates {
                        if carrier.is_some() {
                            break;
                        }
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
                    // The pawn's gameplay rows: merged traits (constant binds derive —
                    // trait-lights F5) + stored conditions from the payload sidecar,
                    // needs from the `needs` sub-table (stat-model F1/F2).
                    let (trait_rows, cond_rows) = pawn_gameplay_rows(&bundle, &pawn, target);
                    let need_rows: Vec<(u64, u16)> = pawn
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
                        &bundle, &iname, &trait_rows, &need_rows, &cond_rows, &active, effect_tic,
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
                            .queue_at(vec![PROMOTE_EVENT, MOVE_TO, target, d], effect_tic, 0)
                        {
                            tracing::warn!(%err, tic = t, "intent walk seed queue failed — queue dropped");
                            intent_queues.remove(&target);
                        } else {
                            let _ = event
                                .reducers()
                                .queue_at(q.fan_program(target), effect_tic, 0);
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
                        if let Err(err) = event.reducers().queue_at(prog, fire_tic, 0) {
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
                                .queue_at(q.fan_program(target), tic_add(master, 4), 0);
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
                                .queue_at(q.fan_program(target), tic_add(master, 4), 0);
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
                    // attack F2: `satisfy.target` may name ANOTHER pawn (`@target` — the
                    // victim): the eval then reads THAT pawn's rows and the SET_NEED
                    // lands on it, so the need-write trigger sweeps the VICTIM (a fatal
                    // bite fires the bunny's death, not the wolf's).
                    if let Some(satisfy) = &params.satisfy {
                        let sat_target = match &satisfy.target {
                            Operand::Input(i) => inputs[*i],
                            _ => target,
                        };
                        let (s_traits, s_conds, s_needs);
                        let (eval_traits, eval_conds, eval_needs) = if sat_target == target {
                            (&trait_rows, &cond_rows, &need_rows)
                        } else {
                            let (tr, cr) = pawn_gameplay_rows(&bundle, &pawn, sat_target);
                            s_traits = tr;
                            s_conds = cr;
                            s_needs = pawn
                                .db()
                                .needs()
                                .iter()
                                .filter(|r| r.entity_reference == sat_target)
                                .map(|r| (r.need, r.set_tic))
                                .collect::<Vec<_>>();
                            (&s_traits, &s_conds, &s_needs)
                        };
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
                        let Some(&(nrow, set_tic)) =
                            eval_needs
                                .iter()
                                .find(|(r, _)| resonantdust_codec::object::row_reference(*r) == need_ref)
                        else {
                            reject("target carries no row for the need");
                            continue;
                        };
                        let (_, need_name) =
                            bundle.gameplay_lookup(need_ref).expect("resolved above");
                        let value = f64::from(resonantdust_codec::value::dequantize(
                            row_data(nrow),
                            np.min as f32,
                            np.max as f32,
                        ));
                        let windows = resonantdust_content::needs_eval::rate_windows(
                            &bundle, &need_name, eval_traits, eval_conds, eval_needs, set_tic,
                        );
                        let now_sat = resonantdust_content::needs_eval::satisfaction_at(
                            value, set_tic, &np, effect_tic, &windows,
                        );
                        let (lo, hi) = resonantdust_content::needs_eval::need_bounds(
                            &bundle, &need_name, &np, eval_traits, eval_conds, effect_tic,
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
                            sat_target,
                            need_ref,
                            u32::from(q),
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
                    // The free-count row builder (inventory F1/F3): the need counts FREE
                    // slots, so a mutation writes `effective max − filled-after`. Computed
                    // from the ROWS (the structural truth) so any drift self-heals at the
                    // next mutation (I2/I3).
                    let inv_free_row = |filled_after: usize| -> Option<(u32, u16)> {
                        let nref = bundle.gameplay_reference("need", "inventory")?;
                        let np = bundle.need_params_by_ref(nref)?;
                        let (_, hi) = resonantdust_content::needs_eval::need_bounds(
                            &bundle, "inventory", &np, &trait_rows, &cond_rows, effect_tic,
                        );
                        let free = (hi - filled_after as f64).max(0.0);
                        let q = resonantdust_codec::value::quantize(
                            free as f32,
                            np.min as f32,
                            np.max as f32,
                        );
                        Some((nref, q))
                    };
                    // The STORE effect (inventory F4): the carrier leaves the world through
                    // the destroy tombstone lane, its kind def lands in the acting pawn's
                    // first free slot (INV_ADD), and the free-count SET_NEED rides the SAME
                    // program (F3/I3 — one author, one atomic program). The carrier probe
                    // above already re-validated presence, so a raced pick_up's second
                    // completion never reaches here (I7).
                    let mut stored: Option<String> = None;
                    if params.store.as_deref() == Some("carrier") {
                        let (cpos, is_thing, cold_row, ckind) = carrier;
                        if !is_thing {
                            reject("store names a TILE carrier — only things are portable (inventory F4)");
                            continue;
                        }
                        // The REGISTRY's def for the carrier's kind (newest version,
                        // variant 0 — the name lookup rule the npc/browser use). The
                        // worker's bundle has no registry; the `definitions` table does.
                        let item_def = bundle.thing_name(ckind >> 4).and_then(|n| {
                            index
                                .db()
                                .definitions()
                                .iter()
                                .filter(|d| {
                                    d.kind == n && d.type_name != "gameplay" && d.variant == "0"
                                })
                                .max_by_key(|d| d.version)
                                .map(|d| d.id)
                        });
                        let Some(item_def) = item_def else {
                            reject("store: the carrier's kind has no registry def");
                            continue;
                        };
                        // The structural room check beside the affordance (I2): the need
                        // gated at queue AND at this re-validation, but two SAME-TIC
                        // completions read the same table snapshot — the pass-local adds
                        // count closes that window.
                        let filled = pawn
                            .db()
                            .inventory()
                            .iter()
                            .filter(|r| r.entity_reference == target)
                            .count()
                            + inv_adds_this_pass.get(&target).copied().unwrap_or(0);
                        let Some(free_row) = inv_free_row(filled + 1) else {
                            reject("store: the pawn carries no inventory need");
                            continue;
                        };
                        let (_, hi) = {
                            let nref = bundle.gameplay_reference("need", "inventory").unwrap();
                            let np = bundle.need_params_by_ref(nref).unwrap();
                            resonantdust_content::needs_eval::need_bounds(
                                &bundle, "inventory", &np, &trait_rows, &cond_rows, effect_tic,
                            )
                        };
                        if (filled as f64) >= hi {
                            reject("store: no free slot (inventory I2 — the rows are full)");
                            continue;
                        }
                        *inv_adds_this_pass.entry(target).or_insert(0) += 1;
                        let cell = u32::from((position_micro(cpos) >> 8) as u8);
                        program.extend_from_slice(&[
                            PROMOTE,
                            SET,
                            cold_row,
                            u32::from(TYPE_BIOME_THING),
                            cell,
                            0,
                            0,
                        ]);
                        program.extend_from_slice(&[INV_ADD, target, item_def]);
                        program.extend_from_slice(&[
                            PROMOTE, SET_NEED, target, free_row.0, u32::from(free_row.1),
                        ]);
                        stored = bundle.thing_name(ckind >> 4).map(|s| s.to_string());
                    }
                    // The SPAWN effect: a NAMED thing (food-chain F5/F6 — `on` = the target
                    // pawn's floor cell, `adjacent` = the CARRIER's 3×3 first EMPTY pathable
                    // cell, all full = a logged no-yield) or CARRIED (inventory F5 — the
                    // validated SLOT's kind beside the pawn, REFUSING when no cell exists:
                    // the item stays held, I10). The emptiness/pathability probes are the
                    // SAME pass-level ones movement uses (I5).
                    let adjacent_empty = |around: u32| -> Option<u32> {
                        let (cx0, cy0) = position_to_tile(around);
                        for oy in -1i32..=1 {
                            for ox in -1i32..=1 {
                                if ox == 0 && oy == 0 {
                                    continue;
                                }
                                let (cx, cy) = (cx0 + ox, cy0 + oy);
                                let p2 = tile_to_position(cx, cy);
                                let zone = position_macro(p2);
                                let cell = (position_micro(p2) >> 8) as u8;
                                if thing_kind_at(zone, cell).is_some() {
                                    continue; // occupied
                                }
                                if !cell_pathable(cx, cy) {
                                    continue; // water is not a shelf
                                }
                                return Some(p2);
                            }
                        }
                        None
                    };
                    let mut spawn_write: Option<(u32, u32)> = None; // (position, kind_reference)
                    match &params.spawn {
                        Some(resonantdust_content::loader::SpawnEffect::Thing {
                            thing: sp_thing,
                            at: sp_at,
                        }) => {
                            let spawn_kind: u32 = bundle
                                .thing_object_id(sp_thing)
                                .map(|id| u32::from(pack_kind_reference(id, 0)))
                                .unwrap_or(0);
                            if spawn_kind == 0 {
                                tracing::warn!(thing = %sp_thing,
                                    "spawn names a thing the bundle cannot number — skipped");
                            } else if sp_at == "on" {
                                spawn_write = Some((tile_to_position(px, py), spawn_kind));
                            } else if let Some(p2) = adjacent_empty(carrier.0) {
                                spawn_write = Some((p2, spawn_kind));
                            } else {
                                tracing::info!(tic = t, interaction = %iname,
                                    "no-yield — every adjacent cell is full or unpathable (F6)");
                            }
                        }
                        Some(resonantdust_content::loader::SpawnEffect::Carried) => {
                            let Some((slot, item_def)) = slot_ctx else {
                                reject("spawn = \"carried\" outside a slot location (inventory F5)");
                                continue;
                            };
                            // REFUSE-not-swallow (I10): no empty pathable neighbor → the
                            // whole order no-ops and the item STAYS in the slot.
                            let Some(p2) = adjacent_empty(pawn_pos) else {
                                reject("drop refused: no empty pathable cell beside the pawn (inventory I10)");
                                continue;
                            };
                            let kr = u32::from(
                                resonantdust_codec::object::def_kind_reference(item_def),
                            );
                            spawn_write = Some((p2, kr));
                            let filled = pawn
                                .db()
                                .inventory()
                                .iter()
                                .filter(|r| r.entity_reference == target)
                                .count();
                            let Some(free_row) = inv_free_row(filled.saturating_sub(1)) else {
                                reject("drop: the pawn carries no inventory need");
                                continue;
                            };
                            program.extend_from_slice(&[INV_REMOVE, target, u32::from(slot)]);
                            program.extend_from_slice(&[
                            PROMOTE, SET_NEED, target, free_row.0, u32::from(free_row.1),
                        ]);
                        }
                        None => {}
                    }
                    if let Some((p2, kind_ref)) = spawn_write {
                        let cold_row = pack_cold_row_reference(position_macro(p2), 0, 0);
                        let cellw = u32::from((position_micro(p2) >> 8) as u8);
                        program.extend_from_slice(&[
                            PROMOTE,
                            SET,
                            cold_row,
                            u32::from(TYPE_BIOME_THING),
                            cellw,
                            kind_ref,
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
                            cref,
                            u32::from(cp.duration as u16),
                        ]);
                        for m in &cp.needs {
                            if satisfied_need.as_deref() == Some(m.need.as_str()) {
                                continue; // already re-stamped by the satisfy SET_NEED
                            }
                            let Some(mref) = bundle.gameplay_reference("need", &m.need) else {
                                continue;
                            };
                            let Some(mp) = bundle.need_params_by_ref(mref) else { continue };
                            let Some(&(mrow, mset)) = need_rows
                                .iter()
                                .find(|(r, _)| resonantdust_codec::object::row_reference(*r) == mref)
                            else {
                                continue; // the pawn doesn't carry this need — nothing to stamp
                            };
                            let mvalue = f64::from(resonantdust_codec::value::dequantize(
                                row_data(mrow),
                                mp.min as f32,
                                mp.max as f32,
                            ));
                            let mwindows = resonantdust_content::needs_eval::rate_windows(
                                &bundle, &m.need, &trait_rows, &cond_rows, &need_rows, mset,
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
                                mref,
                                u32::from(mq),
                            ]);
                        }
                    }
                    if let Err(err) = event.reducers().queue_at(program, effect_tic, 0) {
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
                                .queue_at(q.fan_program(target), effect_tic, 0);
                        }
                        // The REMOVE effect (food-chain F5): death's second half — the
                        // pawn shard deletes the entity's rows (StateGone fans, clients
                        // drop the mover; idempotent — I3) and the ephemeral intent
                        // queue clears (I7). The meat SET rides the queued program.
                        if params.remove.as_deref() == Some("target") {
                            if let Err(err) = pawn.reducers().remove(self_ref, t, target) {
                                tracing::warn!(%err, tic = t,
                                    pawn = format!("{target:#010x}"), "remove failed");
                            } else {
                                intent_queues.remove(&target);
                                tracing::info!(tic = t, pawn = format!("{target:#010x}"),
                                    "the holder died — removed (food-chain F5)");
                            }
                        }
                        tracing::info!(tic = t, effect_tic, interaction = %iname,
                            target = format!("{target:#010x}"),
                            satisfied = ?satisfied_need, from = log_from, to = log_to,
                            moved = params.move_effect.is_some(),
                            destroyed = params.destroy.is_some(),
                            yielded = ?yielded,
                            stored = ?stored,
                            slot = ?slot_ctx.map(|(s, _)| s),
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
                            // The shared route's FIRST CHORD, one STRIDE along it — now
                            // `move_eval::next_hop` (shared-simulation P1). The chord
                            // recompute, the off-centre `clear_point_fraction` clamp and
                            // the lattice recenter all live there, so an observer asking
                            // "where will this pawn be" runs THIS code, not a copy of it.
                            use resonantdust_codec::object::{point_to_position, position_to_point};
                            match resonantdust_content::move_eval::next_hop(
                                position_to_point(p.position_reference),
                                position_to_point(*dest),
                                (ctx.tics_per_tile)(*obj),
                                ctx.pathable,
                            ) {
                                Some(hop) => {
                                    p.position_reference =
                                        point_to_position(hop.point.0, hop.point.1);
                                    p.base_tic = ctx.now;
                                    p.data =
                                        pack_pawn_data(hop.facing, pawn_trip_serial(p.data));
                                }
                                None => {
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
fn base_row(
    data: &data_shard::DbConnection,
    pawn: &pawn::DbConnection,
    player_pawn: &player_pawn::DbConnection,
    entity: u32,
    tic: u16,
) -> Option<Base> {
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
        Shard::PlayerPawn => latest_base!(player_pawn),
        // Cold cells (tile/thing) are cold-row-addressed with no per-entity slot — their base is the
        // baseline, read/written by the shards' own reducers.
        _ => None,
    }
}
