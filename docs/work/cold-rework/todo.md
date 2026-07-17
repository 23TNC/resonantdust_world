# Todo — cold-rework

_Planned, not started. Dependency-ordered by phase (P1 gates the shape; P2/P3 stand up the router +
overlay read-path; P4 is the mutation build). All blockers resolved — decisions in
[`forks.md`](forks.md). Move an item to `completed.md` when it lands.
VARIABLES/TABLES already carry the target shape._

**P1 is done** (build gates green; browser confirm pending a redeploy) — see
[`completed.md`](completed.md).

---

**P2 is done** (routing directory + seed + edge resolution; multi-shard connection pool deferred) — see
[`completed.md`](completed.md).

---

## P3 · Overlay read-path — `state`/`state_log` on cold (shared macro)

Give cold the hot pair and composite it, before anything writes it. Via the shared macro ([F1]).

- **step 1 — extract `tick_pipeline!`**: lift `data_shard`'s `clock`/`state_log`/`state` +
  `init`/`bump`/`claim`/`write`/`gc` into a shared macro crate; reduce `data_shard` to the payload +
  the invocation. **Prove byte-identical** — regenerate bindings (clean diff), the wolf still moves —
  *before* anything else builds on it. (Re-touches the live pipeline.)
- **step 2 — cold invokes it**: `tile`/`thing` invoke `tick_pipeline!` (same three-ref payload) +
  keep their baseline tables. No writes yet beyond `init`/`seed`.
- **edge**: also subscribe a zone's cold `state` (`WHERE macro_position_reference = <zone>`) and relay
  it; a `ColdState` frame carries the per-entity override.
- **client**: composite **baseline ⊕ state** — index `state` rows by `position_reference`; a cell with a
  `state` override renders from `state` (or hides, if removed), else from the baseline.

**Done when:** `data_shard` is macro-generated and unchanged (wolf moves); and with a hand-inserted cold
`state` row the client shows the override in place of the baseline cell, reverting when it's dropped —
the composite is correct even though nothing yet *produces* the row.

---

## P4 · Mutation — mint → compose → fold (the large build)

Decisions settled ([`forks.md`](forks.md) F1/F2). Builds on P2's router + P3's overlay.

- **server_reference (master-assigned, [F2])**: each cold shard gets a `server` table set by the master
  at standup (like `set_orchestrator`); the mint reads it. (`event_shard`'s hardcoded const migrates to
  this pattern as a small follow-up.)
- **mint (`UNPACK`)**: a cold shard mints an `entity_reference` (`server_reference` + counter, the
  `event_shard` pattern) for a touched cell and writes its first `state_log` row; deterministic-from-event.
- **compose**: events targeting the cell run on the assigned worker composing cold `state_log` exactly
  like `data_shard` (same macro); `promote_state` publishes to `state` (throttled — movement fans out
  start/end only).
- **route ([F2])**: the edge routes an entity's `state` by `entity_reference`'s `type_id` → region →
  shard through the **same `index.cold_shards`** lookup P2 built.
- **fold (`PACK`/GC)**: GC queues a fold — write the settled cell back into the baseline, drop its
  `state` row, tombstone (not drop) its `state_log` rows. Removal = a `state_log`/`state` row with the
  removed marker (no `cold_removed` table).
- **core two-phase**: core issues `UNPACK` at N=0 (pre-empted), learns the id by watching the position
  in `state`, issues the op gated on the row.

**Done when:** a scripted mutation (e.g. decrement a thing's `count`, or remove a tile) mints, composes,
promotes, renders on the client, and GC folds it back into the baseline with the `state` row dropped —
all through events, no direct cold write.
