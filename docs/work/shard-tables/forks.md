# Forks — shard-tables

_Decision points, options, what we chose, why. (The design-shaping F4–F6 live in
[`cold-rework/forks.md`](../cold-rework/forks.md); this stream's build-time decisions are here.)_

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
