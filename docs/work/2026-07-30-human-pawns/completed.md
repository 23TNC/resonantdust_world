# Completed — human-pawns

## 2026-07-30 — P0 complete: packed defs + the payload sidecar live in spacetime

**Packed pawn defs end-to-end.** `resolve_thing` now returns the packed
`TYPE_PAWN | species | kind | variant` def (species read off the kind's texture stem
through the new code-owned palette `codec::object::pawn_species_subtype_id` — comment
there explains why NOT the alphabetical manifest subcategories); the worker's `tics_for`
keys packed defs by `def_kind_id` (legacy raw ids still key as-is); the wolves brain
adopts by `def_sans_variant` (new codec helper); MoverLayer decodes the kind from the
packed def's kind half (nonzero high half discriminates packed vs legacy). Verified: all
builds green (codec 60 tests, core 25, worker/orch/master/npc/edge/wasm/webgl-tsc), and
the live soak — the npc resolved `def=0x30010070` and its CREATE minted wolf
`0x30800001`, adopted + wandering on schedule (~2 s/tile trips).

**The sidecar pair, slaved via a macro hook.** `entity_tables!` gained an optional
`state_hook: fn` arm (no-op stamped otherwise; data_shard rebuilt unchanged); the pawn
module declares `payload_log` + `payload` (shapes in `TABLES.md`) and hooks
`payload_follow_state` — the zone re-key rides INSIDE every `entity_state` upsert
transaction. `spawn` gained `payload: Vec<u32>` and writes both sidecar rows in the
mint transaction (empty payload ⇒ no rows). Encoding lives in the new
`shared/codec/src/payload.rs` (`PAYLOAD_OP_PART`, `part_entry`, `payload_parts` +
4 tests: round-trip, unknown-op skip, malformed tail, empty).

**Hops are payload-free STRUCTURALLY** — the worker contains zero sidecar code (grep
clean; I1 records the acceptance re-scope from the planned worker unit test).

**CREATE is variable-arity + type-routed.** `def position count payload×count` (the
`count` word third, same slot as INIT_ZONE's); framed in `codec::action::program`,
excluded from the write/read sets and routes (3 new codec tests incl. framing a trailing
instruction past the payload); the worker's arm routes by `def_type_id` — TYPE_PAWN
spawns, anything else warns by name WITHOUT eating the pending PROMOTE latch reset;
`ACTIONS.md` updated (CREATE = THE creation verb; INIT_ZONE's "one variable-arity verb"
claim corrected). The npc emits `count 0`.

**The fan.** Edge: `ServerMsg::Payload` frame, `payload` joined into the per-zone pawn
subscription (+ on-applied replay for resting pawns) and insert/update callbacks. Core:
mirrors the frame, decodes `PART` entries → `Event::PawnParts` (engine + web transports);
wasm boundary ships flattened (slot, def) pairs; `WasmClient.onPawnParts` exposes them.
Unit: `protocol::tests::a_payload_frame_decodes_to_part_slots` (wire JSON → both slots).

**The live drill** (after `rd redeploy --run --no-reset` — data PRESERVED — + sim-crate
rebuild/restart + legacy raw-def wolf rows deleted by SQL): a marker pawn minted from the
BROWSER (`__bridge.client.queue`) with `PART(0, 0x32001237)` + `PART(1, 0x3200123B)` —
sidecar row landed byte-identical in spawn zone 83, tic 20396; the browser's
`onPawnParts` log received both slots; `moveEntity` walked it (94,56)→(98,56) ACROSS the
zone-83→99 boundary — after arrival the payload row read back **byte-identical, zone
re-keyed 83→99, tic still 20396** (content-change tic preserved). A `TYPE_THING` def
CREATE logged `CREATE rejected: no shard arm for this def type` and minted nothing.
Drill pawn deleted (all four tables); the wolf kept walking throughout.
