# Forks — shard-tables

_Decision points, options, what we chose, why. (The design-shaping F4–F6 live in
[`cold-rework/forks.md`](../cold-rework/forks.md); this stream's build-time decisions are here.)_

---

## F9 · P3 converts the shards to the macros but keeps *adapted* direct reducers; events wait for P4 (2026-07-18)

**Context.** Converting `tile`/`thing` to `dense_entity_tables!() + overlay_tables!()` (P3) vs. also
making all writes event-driven / retiring `seed`/`set_*`/`fold` (P4) — do them together or separately?

**Options.** (A) P3 = tables + rename + client render, with `seed`/`set_tile`/`fold` *adapted* to the
new `entity_state` (dense baseline) + `overlay` tables but still **direct reducers**; P4 then retires
them for events. (B) Merge P3+P4 — convert *and* go event-driven in one pass.

**Decision: (A).** Smaller, verifiable steps. P3's win is the table shapes + a client that renders
`entity_state ⊕ overlay` (and, crucially, lets me confirm the dense/overlay macros work end-to-end).
The adapted `seed`/`set_tile`/`fold` are throwaway (P4 replaces them with `init_zone`/`SET`→overlay/
`PACK` events), but the throwaway is small next to the risk of a merged tables+events+client change
landing unverifiable. P3 does **not** fix the acquire race (that's P6, when the baseline goes
event-driven + promoted) — it's the plumbing P4/P6 build on.

**Tile conversion shape (P3).** `entity_tables!(data:u8)` (old hot overlay) + `cold_tile` (baseline) →
`dense_entity_tables!()` (baseline `entity_state`, `items: Vec<DenseItem{kind_reference}>`) +
`overlay_tables!()` (`overlay`, `items: Vec<OverlayItem>`). `seed` writes `entity_state` (dense);
`set_tile` writes an `overlay` item; `fold` folds `overlay` → `entity_state`. `mint`/`mint_counter`
drop (cold is `cold_row_reference`-addressed, no per-cell entity mint). `thing` mirrors with
`sparse_entity_tables!`. The macros' own `claim`/`write`/`gc` stay dead until P4 wires the worker.

---

## F10 · P3 edge relay is **wire-transparent** — the client is unchanged (2026-07-18)

**Context.** The tile/thing baseline is now `entity_state` (dense/sparse `items`) and the override is a
whole-row `overlay` (`items: Vec<OverlayItem>`, `cold_row_reference`-addressed, **no per-cell
entity_reference**). The old wire is `ColdTile{tiles:Vec<u16>}` / `ColdThing{things:Vec<u32>}` +
per-cell `ColdState{entity_reference, position_reference, tic, …}`. Do we change the wire (and rework
the pixijs client to composite `entity_state ⊕ overlay` natively) or keep it?

**Options.** (A) **Wire-transparent** — the edge re-packs `entity_state.items` → the existing
`ColdTile.tiles`/`ColdThing.things` and re-frames each `overlay` item → the existing per-cell
`ColdState` (synthesizing `position_reference` from `(macro, tile_reference, layer)` and a stable
pseudo-id via `object::cold_entity_reference`). **Client + protocol untouched.** (B) New wire
(`ColdOverlay{items}`, retire `ColdState`) + rework the client composite (drop the `tic`/
`cold_entity_reference` path, whole-row overlay wins by cell).

**Decision: (A) for P3; (B) deferred to P3′.** Wire-transparency makes P3 a **single edge-side,
deployable, browser-verifiable** increment — it proves the dense/sparse/overlay macros end-to-end (the
[F9](#) win) against an *unchanged* client, the safest way to confirm the table conversion. The
client composite-simplification (todo P3 bullet 4 — drop the per-cell `tic` composite +
`cold_entity_reference`, move to whole-row `overlay`) is orthogonal and lands cleaner against a proven
backend, as **P3′**. Cost: the edge synthesizes a stable per-cell `entity_reference`
(`cold_entity_reference(server, position)`) + `position_reference` for each overlay item to fit
`ColdState` — throwaway that P3′ deletes. The `on_applied` baseline snapshot relay (P6 band-aid) stays,
now iterating `entity_state` instead of `cold_tile`.

---

## F7 · The four table macros are **parallel siblings**, not a shared inner macro (2026-07-18)

**Context.** `entity_tables!`, `dense_entity_tables!`, `sparse_entity_tables!`, `overlay_tables!` share
an identical **composition core** — the `clock`, the `uid` (`subject:32 | tic:16`), the
`worker_reference`/`observer_reference`/`dirty`/`status` columns, and the `claim`/`write`/`gc`/`promote`
reducers. Only the **row shape** (identity + payload columns) and the **table names** differ. The DRY
instinct is a private inner `__compose!` macro the four wrappers invoke.

**Blocker to that (from [P0](blockers.md) B-2).** SpacetimeDB's `#[table(accessor = …)]` **rejects a
`macro_rules!` metavariable in the accessor** (`cannot find value _table_name`). A shared inner macro
would receive each table's accessor as a parameter → a metavariable at the `#[table]` site → breaks.
The accessor **must be a literal at the `#[table]` site**, so the table definitions (and the
`ctx.db.<accessor>()` calls that key off them) can't be emitted by a parameterized inner macro.

**Decision.** The four macros are **parallel siblings in `codec/pipeline.rs`** — each emits its own
`#[table]` blocks + reducers with literal accessors, differing only in row shape + names. This is still
**DRY where it counts**: a shard just *invokes* a macro (no hand-copied composition code per module —
the thing `tick_pipeline!` was created to kill). The parallelism is four adjacent macros in **one
file**, structurally identical, easy to diff — not N copies scattered across shard modules.

**Shape.** All cold macros are `cold_row_reference`-addressed (identity `cold_row_reference` +
`macro_position_reference` + `subtype_id` + `layer_id`), `tic`, then `items: Vec<DenseItem<T>>` /
`Vec<SparseItem<T>>`; `uid` = `cold_uid` (`cold_row_reference:32 | tic:16`, same packing as
`state_uid`). `entity_tables!` (hot) stays as-is — its parallel structure is the template.

**Coexistence constraint (found while authoring).** A cold shard invokes **two** composition macros —
the baseline (`dense_`/`sparse_entity_tables!`) *and* `overlay_tables!` — on the same module. If both
emit `Clock` / `bump` / `init` / `claim` / `write` / `gc`, they **collide** (duplicate tables +
reducers won't compile). So:
- **The baseline macro owns the shard's `Clock` + `bump` + `init`** (one clock per shard) and emits
  `claim` / `write` / `gc` + `entity_state` / `entity_state_log`.
- **`overlay_tables!` is clock-less** and emits **distinctly-named** reducers — `claim_overlay` /
  `write_overlay` / `gc_overlay` + `overlay` / `overlay_log`. The worker calls `write_overlay` for an
  override, `write` for a baseline (routed by which it's composing). A hot shard (`data_shard`) has no
  overlay, so no collision there. `overlay_tables!` ≈ `sparse_entity_tables!` minus the clock, with the
  `_overlay` reducer/table names.
