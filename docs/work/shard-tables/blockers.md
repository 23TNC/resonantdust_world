# Blockers — shard-tables

_Things needing human input or a decision before / during the build. Newest-first._

---

## B-1 · Design-lock + go-ahead — ✅ RESOLVED (2026-07-18): proceed with the tables

The "cleanup passes" are **intentionally deferred until the tables are done** — they mostly depend on
this rework. So the tables stream is cleared to proceed; the follow-on concerns are the *next* streams
(recorded in [`README` §Follow-on](README.md)), not gates on this one. The follow-ons (top-of-head):
client's data interpretation (needs tables nailed first), tiles-not-loading (may be *fixed* by this
rework — P6 verifies), the **~10,000 draw calls** perf problem, moving wolves to a **`pawn` table** (off
`data_shard`), and standing up **hot *and* cold thing shards** — the last two **enabled by** these macros.

## B-2 · Payload-generic `#[table]` — ✅ CONFIRMED by spike (2026-07-18)

A throwaway macro stamped, from **one invocation**, both `spike_with` (payload `data: u8` spliced) and
`spike_without` (no payload) — both compiled to wasm and generated correct SDK bindings
(`spike_with_type.rs` has `data`, `spike_without_type.rs` doesn't). Payload-as-macro-parameter works,
exactly as reasoned (compile-time splice; the `#[table]` proc-macro sees a concrete struct). Spike
reverted.

**One caveat found (not a problem for us):** a `macro_rules!` **metavariable in the `#[table(accessor =
$x)]` attribute** breaks the proc-macro (`cannot find value _table_name`). The real macros use a
**fixed** table name each (`entity_state`, `overlay`), so the accessor is always a literal — only the
*payload* varies. Don't parameterize a table's name/accessor through a metavariable.

## Not blockers — decide-and-proceed (defaults, override if you disagree)

- **Seed content source (P5)** — worker-side worldgen vs an event carrying the `Vec`. Default:
  **worker-side** (small event, replay-deterministic from worldgen). Needs the worldgen crate reachable
  from the worker — an internal refactor, not a blocker.
- **`SET` addressing (P4)** — the as-built `SET` targets a per-cell `cold_entity_reference`; the overlay
  design targets the biome-row (`cold_row_reference`) + a `tile_reference` operand. Default: **change
  `SET`'s signature** to the row+cell form. Reversible.
- **P1 risk** — converting the live `data_shard` is the scariest step; the plan gates it behind a
  **behavior-preserving proof** (byte-identical bindings + wolf still moves) before any rename, exactly
  as the `tick_pipeline!` extraction was proven. Managed by sequencing, not a blocker.
