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

**P3 is done** (steps 1–2 + the overlay read path — all live-verified) — see [`completed.md`](completed.md).

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
