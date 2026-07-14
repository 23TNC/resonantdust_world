# Cross-cutting feature intent (staging)

_Last updated: 2026-07-14._

Features that touch **many components** — their intent belongs in the `intent/` of each component
they touch. When we don't yet know which components a feature touches, or lack enough information
to ascribe it, it's **parked here** until scoped. As a feature gets scoped, its intent
**distributes out** into the affected components' `intent/` folders (leaving a pointer here, or
retiring the entry). See [`../CONVENTIONS.md`](../CONVENTIONS.md) § `docs/intent/`.

This is where a half-formed "we want X to…" lives before it's a per-component contract.

## Staged features

- **[pathfinding](pathfinding/README.md)** — a **DESIGN** (written against the 0.2.3 pipeline): a
  *deterministic* `path()` in `shared` makes the move-**intent** the position — store the intent,
  derive `tile_at(tic)` identically on worker + every client (no per-tile rows, no jumps). Splits
  `ACTION_MOVE` → `place` (absolute, today's behavior) + `move` (pathfind intent). **Not built.**
  Touches shared/tick + worker + client. To distribute into those component intents once built.
- **[sync](sync.md)** — client/server sync: authoritative bitemporal event log + deterministic
  projection (synced clock + shared render delay + interpolate-by-`valid_at`). Cross-cutting:
  client/core + pixijs + edge + worker + shard all realize it. Parts are implemented (the event
  log / state model); the doc predates the event-DSL rewrite (`free_things` vocabulary) and should
  be re-confirmed + distributed into the component intents.
