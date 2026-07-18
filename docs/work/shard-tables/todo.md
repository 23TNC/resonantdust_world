# Todo — shard-tables

_Phased build of the `*_tables!` generalization. Items move to `completed.md` as they land + verify.
Design: [`README`](README.md) · [`TABLES.md`](../../TABLES.md) · [`ACTIONS.md`](../../ACTIONS.md)._

---

## P0 · Spike — the payload-generic `#[table]` macro (**gates everything**) — ✅ DONE

- [x] Proved: one macro invocation stamps a `#[table]` **with** a payload (`data: u8`) and **without** —
      both compile to wasm + generate correct bindings ([`blockers.md` B-2](blockers.md)). No fallback
      needed. **Caveat:** a table's name/accessor must be a **literal**, not a metavariable (the real
      macros have fixed names, so fine).

## P1 · The macros in `shared/codec` (behavior-preserving on `data_shard`)

- [x] **Payload is a parameter** — `tick_pipeline!` now takes the composed payload as a macro arg,
      spliced into `TargetState`/`state_log`/`state`. Proven **byte-identical** (empty bindings diff)
      + `--keep` redeploy + a mover composes at runtime. See [`completed.md`](completed.md).
- [x] **Renamed** `tick_pipeline!` → `entity_tables!`; tables/structs `state`/`state_log` →
      `entity_state`/`entity_state_log`. Swept codec + 3 modules + worker + edge + bindings (st +
      edge); the client wire (`ServerMsg::State`) is untouched. Runtime-verified — a mover composes into
      `entity_state`. (Reset-publish needed — a rename is an incompatible migration; **rebuild the sim
      binaries after**, not just check.) See [`completed.md`](completed.md).
- [ ] Split `position_reference` → first-class `macro_position_reference` + `micro_position_reference`
      columns (the design's fixed columns; `data` stays the generic payload). Updates worker/edge/client
      to the split — a runtime-preserving (not byte-identical) step, verify **wolf still moves**.
- [ ] Author `dense_entity_tables!` / `sparse_entity_tables!` — payload = `Vec<DenseItem<T>>` /
      `Vec<SparseItem<T>>`, keyed by `cold_uid`. And `overlay_tables!` (= sparse, named `overlay`).

## P2 · The `PROMOTE` prefix + smart-atomic promote

- [ ] codec: `PROMOTE` action (value 1, arity 0) — retire `PROMOTE_STATE`/`PROMOTE_COLD`.
- [ ] worker `apply`: on `PROMOTE`, set a pending bit; the next action marks its write targets to
      promote (in scratch); pass the bit to the write reducer.
- [ ] macro `write` reducer: when promote is set, copy `entity_state`←`entity_state_log` **and**
      `overlay`←`overlay_log` where `visible.tic != log.tic`, **in one transaction**.
- [ ] Verify: `promote place <mover> <dest>` still lands a mover in `entity_state`.

## P3 · Convert the shards + rename tables

- [ ] `data_shard` → `entity_tables!{data:u8}`: `state`/`state_log` → `entity_state`/`entity_state_log`.
- [ ] `tile` → `dense_entity_tables!() + overlay_tables!()`; `thing` →
      `sparse_entity_tables!{data:u8} + overlay_tables!{data:u8}`. `cold_tile`/`cold_thing` → the dense/
      sparse `entity_state`; add `overlay`/`overlay_log`. Regenerate bindings (edge + st-bindings).
- [ ] Edge relay: `entity_state` + `overlay` per zone (fold the two subscriptions; keep the
      `on_applied` snapshot relay until P6).
- [ ] Client render: `entity_state ⊕ overlay` (overlay cell shadows baseline) — **drop** the
      `cold_entity_reference` id path + the per-cell `tic` composite (`baselineSuppressed`); an overlay
      cell simply wins.
- [ ] Reset + reseed (dev); browser: terrain + overrides render on the new tables.

## P4 · Events replace direct writes

- [ ] `init_zone macro` action — worker builds the whole baseline row in scratch, writes
      `entity_state_log`; `promote init_zone` projects it. Edge **queues** this instead of calling
      `seed`. Retire `seed`.
- [ ] Per-cell override → `overlay_log` (reconcile `SET` to the biome-row + `tile_reference` addressing
      — [`ACTIONS.md`](../../ACTIONS.md) `SET` note). Retire `set_tile`/`set_thing`.
- [ ] `PACK` action — worker folds settled `overlay` cells into `entity_state_log`; GC queues
      `promote pack …` (atomic promote of folded baseline + cleared overlay). Retire `fold`.
- [ ] Worker: compose a **cold row** — whole-`Vec` scratch, block/write/promote like a hot entity.

## P5 · Seed content source

- [ ] Resolve F5/F6 open item b (worker-side worldgen vs event-carried `Vec`); implement for
      `init_zone`. Leaning worker-side (small event, replay-deterministic from worldgen).

## P6 · Verify + retire the band-aid

- [ ] Browser: a **fresh world under fast panning** renders fully-covered terrain — **no acquire race**
      (the bug that started this). A scripted override renders → `PACK` → **atomic promote, no flash**.
- [ ] Remove the `on_applied` relay band-aid (edge) once the event path is proven.
- [ ] Update `completed.md` + the memory; mechanical `cold_row_reference`→ rename left for a later pass.

---

**Done when:** the [`README` invariant](README.md) holds — every shard on the macros, all writes through
events + `PROMOTE`, `entity_state ⊕ overlay` rendering with no acquire race, browser-verified.
