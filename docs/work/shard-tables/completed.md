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
correctly split). Browser regression (terrain renders, no errors) pending.
