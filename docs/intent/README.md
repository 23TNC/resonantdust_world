# Cross-cutting feature intent (staging)

_Last updated: 2026-07-14._

Features that touch **many components** — their intent belongs in the `intent/` of each component
they touch. When we don't yet know which components a feature touches, or lack enough information
to ascribe it, it's **parked here** until scoped. As a feature gets scoped, its intent
**distributes out** into the affected components' `intent/` folders (leaving a pointer here, or
retiring the entry). See [`../CONVENTIONS.md`](../CONVENTIONS.md) § `docs/intent/`.

This is where a half-formed "we want X to…" lives before it's a per-component contract.

## Staged features

- **[spacetime-again](spacetime-again/README.md)** — the shard rebuild: event/data shard split,
  per-worker subscriptions, opt-in promotion. The **flow**; its shapes are
  [`TABLES.md`](../TABLES.md) + [`VARIABLES.md`](../VARIABLES.md). **Nothing built.**
- **[sync](sync.md)** — ⚠️ **stale.** Client/server sync as an authoritative *bitemporal* event log,
  interpolate-by-`valid_at`. `valid_at` was deleted 2026-07-15 and the model is tic-based now; the
  server half was already superseded twice over. What survives is the client half — synced clock,
  render delay `D`, interpolate — and the clock is implemented (`client/core/src/clock.rs`).
  Rewrite against the tic or delete; do not build from it.

- **pathfinding — ☠ GONE (2026-07-15).** Written against the deleted pipeline, in `valid_at` /
  `ACTION_MOVE` vocabulary. `git show checkpoint/pre-shard-rebuild:docs/intent/pathfinding/README.md`
  if the `path()`-as-intent idea is wanted back — the idea was: store the move intent, derive
  `tile_at(tic)` identically on worker and client, no per-tile rows.
