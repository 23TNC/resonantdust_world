# Todo — cold-rework

**P1–P3 done + live-verified; P4 foundation (mint + `set_tile`) done + live-verified** — see
[`completed.md`](completed.md). Decisions in [`forks.md`](forks.md).

---

## P4 · Mutation — the remaining event-driven path

**Both cold types mutate end-to-end, live-verified** — `set_tile`/`set_thing` (biome-preserving mint →
`state` override) → `fold` (PACK back into the baseline). See [`completed.md`](completed.md). What's
left, by kind:

**Event-driven `UNPACK` — DONE + live-verified** (see [`completed.md`](completed.md)). The
deterministic cold entity id ([`deviations.md`](deviations.md) D-1) sidestepped the deferred
spawn-claim: a `SET` action carries the position-derived id as a normal write operand, so the existing
grouping/claim/worker machinery routes it — the worker + orchestrator gained only shard-routing by
`server_reference`. A `SET` queued at the event shard flows orchestrator (claims on the cold shard) →
worker (composes + writes the cold shard) → `state` → edge `ColdState` → **blue water rendered**; the
hot wolf path is unchanged (regression-checked). **Core two-phase collapsed** — the id is
deterministic, so core needn't watch a position to learn it.

**Remaining**
- **Core-side command** — a `client/core` verb that computes `cold_entity_reference(position)` +
  queues the `SET` program (`[SET, id, def, pos, data, PROMOTE_STATE, id]`), so a *client* (not just a
  CLI-queued event) drives a cold mutation. Small — the pipeline behind it is proven. Pixijs may expose
  it (a debug click-to-paint) once core has it.
- **`fold` through events** — GC currently folds via the direct `fold` reducer (proven). Driving the
  fold itself as an event is a later nicety; the fold logic is unchanged.

**Stopgap promotions** (low urgency, recorded)
- `server_reference` **master-assigned** (F2) — the `tile`/`thing` const only bites at multi-shard.
- `state_log` **tombstone-not-drop** in `fold` (design keeps it briefly for a slow reader / takeover).
- `event_shard`'s hardcoded `SERVER_REFERENCE` → the master-assigned pattern.
- edge **multi-region → multi-shard connection pool** (deferred P2; single-shard works today).

**Done when:** a scripted mutation flows edge → orchestrator → worker → mint → compose → promote →
render, and GC folds it back into the baseline with the `state` row dropped — all through events.
