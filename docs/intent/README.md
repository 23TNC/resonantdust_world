# Cross-cutting feature intent (staging)

_Last updated: 2026-07-14._

Features that touch **many components** — their intent belongs in the `intent/` of each component
they touch. When we don't yet know which components a feature touches, or lack enough information
to ascribe it, it's **parked here** until scoped. As a feature gets scoped, its intent
**distributes out** into the affected components' `intent/` folders (leaving a pointer here, or
retiring the entry). See [`../CONVENTIONS.md`](../CONVENTIONS.md) § `docs/intent/`.

This is where a half-formed "we want X to…" lives before it's a per-component contract.

## Staged features

- **[pathfinding](pathfinding.md)** — intent exists (server pathfinds → commits per-tile moves
  with look-ahead); **not implemented**. Touches client / edge / worker / shared-tick / shard.
  Written against the pre-rewrite model; needs re-confirming against the event-DSL architecture.
- **[sync](sync.md)** — client/server sync: authoritative bitemporal event log + deterministic
  projection (synced clock + shared render delay + interpolate-by-`valid_at`). Cross-cutting:
  client/core + pixijs + edge + worker + shard all realize it. Parts are implemented (the event
  log / state model); the doc predates the event-DSL rewrite (`free_things` vocabulary) and should
  be re-confirmed + distributed into the component intents.
