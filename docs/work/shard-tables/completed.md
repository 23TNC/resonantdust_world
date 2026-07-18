# Completed — shard-tables

_Landed + verified, newest last. One increment each._

---

## P0 · Payload-generic `#[table]` — spiked + confirmed

A throwaway macro stamped, from one invocation, `spike_with` (payload `data: u8` spliced) and
`spike_without` (no payload) — both compiled to wasm + generated correct SDK bindings. Payload-as-macro-
parameter works (compile-time splice). Caveat: a table's name/accessor must be a **literal**, not a
metavariable (real macros have fixed names). Spike reverted. See [`blockers.md` B-2](blockers.md).

## P1 (step 1) · `tick_pipeline!` payload is now a **parameter** — byte-identical

First generalization step: the composed payload — `definition_reference` / `position_reference` / `data`
— moved from hardcoded to a **macro argument** spliced into `TargetState` / `state_log` / `state` (in
order, after the fixed composition columns) + `claim` (`Default::default()`) + `write` / upsert
(`r.$pf.clone()`, so a future `Vec` payload works). The macro **requires a `position_reference`** (the
`state` upsert derives `macro_position_reference` from it). All three shards invoke it explicitly:
`tick_pipeline!(definition_reference: u32, position_reference: u32, data: u8)`.

**Verified behavior-preserving:**
- **Byte-identical bindings** — `data_shard` + `tile` + `thing` rebuilt, `git diff` of the generated
  bindings is **empty** (schema + reducer signatures unchanged; only macro-mechanical body changes).
- **Schema-compatible deploy** — redeployed all three `--keep` (no reset); identities preserved (a
  hot-swap, not a wipe), exactly as byte-identical predicts.
- **Runtime** — a `PLACE` queued at the event shard composed a mover fresh (compose-from-empty →
  `PLACE` → promote) into `data_shard.state` at the target position — the full event → claim → worker
  → macro'd `write` → promote path works through the parameterized macro (the "wolf moves" check).

**Remaining in P1:** rename `tick_pipeline!` → `entity_tables!` (+ `state`→`entity_state`); split
`position_reference` → `macro`/`micro` first-class columns; author `dense_entity_tables!` /
`sparse_entity_tables!` / `overlay_tables!` siblings.

## P1 (step 2) · Rename `tick_pipeline!` → `entity_tables!`, `state` → `entity_state`

Mechanical rename to the target names (fixed literals — the accessor can't be a metavariable, P0
caveat). `state`/`state_log` tables + `State`/`StateLog` structs → `entity_state`/`entity_state_log` +
`EntityState`/`EntityStateLog`; `TargetState` kept. Swept: `shared/codec/pipeline.rs`, the 3 modules
(invocation + `tile`/`thing` manual constructions), `worker` (`entity_state_log` subs + accessors +
traits), `edge` (binding accessors/types + `SELECT * FROM entity_state` subs — **surgical**, preserving
axum `State` and the wire `ServerMsg::State`), and regenerated **both** binding sets (edge + st).

**The client wire is unchanged** — `ServerMsg::State` is a protocol frame, not the DB table, so
core/wasm/pixijs need no change.

**Verified:** codec + `data_shard`/`tile`/`thing` build (bindings cleanly renamed — old `state_*.rs`
deleted, new `entity_state_*.rs` written); worker/orchestrator/master `check`; edge builds. Reset-
published the 3 modules (a rename is an incompatible migration) + **rebuilt the sim binaries** (a stale
binary subscribing to the vanished `state_log` panics — caught + fixed). Runtime: a `PLACE` composed a
mover into `entity_state` at the target position ("composed component").


## P1 (step 3) · Split `position_reference` → `macro`/`micro` first-class columns

Reached the design's fixed-column shape: `entity_state`/`entity_state_log`/`TargetState` gain
`definition_reference` + `macro_position_reference` + `micro_position_reference` as **fixed** columns;
`data` becomes the **sole** generic payload — so the contract is `entity_tables!(data: u8)` (a tile
shard would pass none). The macro no longer derives `macro_position` from a payload `position_reference`.

**Kept the client wire unchanged (server-side only):** the worker splits `position_reference` →
`macro`/`micro` when it builds a `TargetState` and reassembles it in `base_row` (from the log's
macro/micro); the **edge reassembles `position_reference = pack(macro, micro)`** in `state_frame` +
both cold-overlay relays, so `ServerMsg::State`/`ColdState` are byte-for-byte the same and core/wasm/
pixijs need no change. (`pack ∘ split = id`, so the wire value is provably identical.)

Swept: `pipeline.rs` (structs + claim/write/upsert), the 3 module invocations + `tile`/`thing` manual
constructions + `fold`/reuse-find (now compare `macro`+`micro`), `worker` (imports + `TargetState`
split + `base_row` reassembly), `edge` (3 reassembly sites), both binding sets regenerated.

**Verified:** codec/modules/worker/orchestrator/master/edge build; reset-published + rebuilt sim; a
`PLACE` composed a mover into `entity_state` with `macro=0, micro=30512` (position 0x7730 = 30512
correctly split). Browser regression: **no console errors**; movers/wire unaffected. Ground was black — but that is the **pre-existing cold acquire race** (`cold_tile` reseeded fine, 11 rows, same relay path as the rendering `cold_thing`), NOT a split regression (the split never touches `cold_tile`). The rework fixes it at P6.

## P1 (step 4) · Author the cold sibling macros — `dense_`/`sparse_entity_tables!` + `overlay_tables!`

The cold composition macros, parallel siblings of `entity_tables!` ([`forks.md` F7](forks.md) — the
`#[table]` accessor can't be a macro param, so no fully-shared inner macro). All `cold_row_reference`-
addressed, `uid` = `cold_uid` (= `pack_state_uid(cold_row, tic)`):

- **`dense_entity_tables!(T)`** / **`sparse_entity_tables!(T)`** — the baseline pair
  `entity_state`/`entity_state_log`, `items: Vec<DenseItem>` (ZONE_DIM², index) / `Vec<SparseItem>`
  (occupied, each carries `tile_reference`). Each **owns the shard's `clock`** + `claim`/`write`/`gc`.
- **`overlay_tables!(T)`** — the override pair `overlay`/`overlay_log`, always sparse; **clock-less**
  with **`claim_overlay`/`write_overlay`/`gc_overlay`** so it coexists with the baseline on one shard
  without table/reducer collisions (F7 coexistence constraint).
- Shared helpers: `__cold_clock!` (the one clock, literal accessor) + `__cold_baseline_tables!($item)`
  (baseline tables + reducers, parameterized only by the *item type* — a param, not an accessor, so
  F7-safe). dense/sparse differ only in the item struct.

**Spike-validated:** `data_shard` (which is *just* the macro) temporarily pointed at
`dense_entity_tables!() + overlay_tables!()` — **compiled to wasm** (both baseline + overlay expand, no
`Clock`/`claim`/`write`/`gc` collision), bindings generated, then reverted clean. `sparse_entity_tables!`
shares `__cold_baseline_tables!`, so validated transitively. Not yet wired to `tile`/`thing` (P3).

## P2 · The `PROMOTE` prefix

`PROMOTE_STATE` (arity 1, explicit target) → **`PROMOTE`** (value 1, **arity 0** prefix). Executing
left-to-right, the worker's `apply` sets a pending bit on `PROMOTE` and the **next** action's write
targets join `promote` (then the bit clears) — so the promote is written as part of that action's
result, no post-pass. `core`'s `move_to_program`/`place_program` emit the prefix (`[PROMOTE, MOVE_TO,
entity, dest]`). codec tests updated + green (48).

**Verified:** `[PROMOTE, PLACE, 0x30000001, 50000]` queued at the event shard flowed orchestrator →
worker → `entity_state` (mover at micro 50000, "composed component"). **Operational lesson:** rebuild
**all** sim binaries after a codec change — the *orchestrator* also frames programs (`write_targets`),
and a stale one mis-frames the new `PROMOTE` arity (`UnknownAction`), silently skipping events.

**Deferred to P4/fold:** the cross-table *smart-atomic* promote (copy `entity_state` **and** `overlay`
in one commit). Per F7 the baseline + overlay are separate reducers (`write` / `write_overlay`), so a
single transaction across both isn't available — a fold's no-flash comes from **ordering** (promote
baseline before clearing overlay) unless a combined fold reducer is added. Decided when `PACK` is built.

## P3 · Convert the shards + rename tables — baseline + edge relay (client-transparent)

`tile` → `dense_entity_tables!() + overlay_tables!()`; `thing` → `sparse_entity_tables!{data:u8} +
overlay_tables!{data:u8}`. The bespoke `cold_tile`/`cold_thing` baseline + the hot-format
`entity_state` overlay + the per-cell entity mint are **gone**: the baseline is now the dense/sparse
`entity_state` (`cold_row_reference`-addressed, owns the shard `clock`), the override is the sparse
`overlay`/`overlay_log`. `seed` fills the baseline; `set_tile`/`set_thing` write an `overlay` cell
(merge by `tile_reference`, `kind==0`=removal); `fold` (PACK) folds the overlay into its baseline
sibling by shared `cold_row_reference`. Direct reducers still (F9) — events are P4.

**Edge relay — wire-transparent (F10).** The baseline callbacks read `entity_state` and re-pack
`items` → the existing `ColdTile.tiles`/`ColdThing.things` wire; the override callbacks read the
whole-row `overlay` and re-frame each cell → the existing per-cell `ColdState` (stable pseudo
`entity_reference` via `cold_entity_reference`, reconstructed `position_reference`; row-delete /
`kind==0` = removal). Zone subscription now `SELECT`s `entity_state` + `overlay`; the `on_applied`
snapshot band-aid (P6) iterates `entity_state`. **Protocol + client untouched.**

**Worker cold path gated (F9 → P4).** tile/thing are now `cold_row_reference`-addressed with no
per-entity slot, so `base_row` is data-only and the `write` routing drops the Tile/Thing arms; cold
writes go through the direct reducers. Nothing live emits `SET`, so the gated path is dead this phase.

**Live-verified (browser).** Full redeploy (tile/thing reset + reseed, edge rebuilt on :8473, all three
sim binaries restarted — worker "subscriptions applied" clean, no schema-mismatch panic). At
`?x=8&y=8`: ground renders across biomes (grass / stone / dirt) from the dense baseline, scatter
(trees) from the sparse baseline, **no black-ground holes inside subscribed zones** (the acquire-race
symptom that started the rework), console clean.

**Remaining:** P3′ — the client composite-simplification (todo P3 bullet 4: drop the per-cell `tic`
composite + `cold_entity_reference`, move to whole-row `overlay`) once wanted; a live `set_tile` check
of the new `relay_*_overlay` path (validated by build + mirrors the proven baseline relay).
