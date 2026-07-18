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
- [x] **Split `position_reference`** → first-class `macro`/`micro` + `definition_reference` columns;
      `data` is the sole generic payload (`entity_tables!(data: u8)`). The worker splits at
      `TargetState` write + reassembles in `base_row`; the **edge reassembles `position_reference` =
      `pack(macro, micro)` for the wire**, so `ServerMsg::State` is unchanged and the **client needs no
      change** (server-side only). Verified: builds green; a mover composed with `macro=0, micro=30512`.
      See [`completed.md`](completed.md).
- [x] **Authored** `dense_entity_tables!` / `sparse_entity_tables!` (baseline, `Vec<DenseItem>` /
      `Vec<SparseItem>`, own the `clock` + `claim`/`write`/`gc`) + `overlay_tables!` (clock-less,
      `overlay`/`overlay_log`, `*_overlay` reducers — F7 coexistence). Shared `__cold_clock!` +
      `__cold_baseline_tables!($item)` (literal accessors — F7). **Spike-validated**: `data_shard`
      temporarily pointed at `dense_entity_tables!() + overlay_tables!()` compiled to wasm (no reducer/
      table collision), reverted clean. Not wired to `tile`/`thing` yet — that's P3.

## P2 · The `PROMOTE` prefix

- [x] codec: **`PROMOTE`** (value 1, **arity 0**) — a prefix, retires `PROMOTE_STATE`. Tests updated
      (`PROMOTE PLACE obj pos` frames; write-set excludes the arity-0 `PROMOTE`).
- [x] worker `apply`: a pending bit set by `PROMOTE`, consumed by the **next** action (its write
      targets join `promote`, then the bit clears). `core`'s program builders emit the prefix form.
- [x] **Verified:** `[PROMOTE, PLACE, 0x30000001, 50000]` queued at the event shard composed the mover
      into `entity_state` at micro 50000 (worker "composed component"). **Rebuild *all* sim binaries
      after a codec change** — a stale orchestrator mis-frames the new arity (caught + fixed).
- [ ] _(Deferred to P4/fold)_ the cross-table **smart-atomic** promote (`entity_state` **and** `overlay`
      in one commit) — a fold concern; separate `write`/`write_overlay` reducers exist (F7), so the
      fold's no-flash comes from **ordering** (baseline before overlay) unless a combined reducer is
      added. Revisit when building `PACK`/fold.

## P3 · Convert the shards + rename tables — ✅ DONE (see `completed.md`)

- [x] `data_shard` → `entity_tables!{data:u8}` (P1).
- [x] `tile` → `dense_entity_tables!() + overlay_tables!()`; `thing` →
      `sparse_entity_tables!{data:u8} + overlay_tables!{data:u8}`. `cold_tile`/`cold_thing` → the dense/
      sparse `entity_state`; `overlay`/`overlay_log` added. Bindings regenerated (edge + st-bindings).
- [x] Edge relay: `entity_state` (baseline) + `overlay` (override) per zone, **wire-transparent** (F10 —
      re-packs to the existing `ColdTile`/`ColdThing`/`ColdState` wire, so the client is unchanged); the
      `on_applied` snapshot relay now iterates `entity_state` (kept until P6).
- [x] Worker cold path **gated** to P4 (F9): `base_row` data-only, `write` routing drops Tile/Thing —
      cold writes go through the direct reducers; nothing live emits `SET`.
- [x] Reset + reseed (dev); browser at `?x=8&y=8` — terrain (grass/stone/dirt) + scatter render on the
      new tables, no in-zone black holes, console clean.

## P3′ · Client composite-simplification (deferred — F10)

- [ ] Client render → **native** `entity_state ⊕ overlay` (whole-row overlay wins by cell): retire the
      per-cell `ColdState` wire + the `tic` composite (`baselineSuppressed`) + the `cold_entity_reference`
      id path; add a `ColdOverlay{items}` frame. Do once wanted — the wire-transparent P3 works today.
- [ ] Live `set_tile` check of the new `relay_tile_overlay`/`relay_thing_overlay` path (build-validated;
      mirrors the proven baseline relay).

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
