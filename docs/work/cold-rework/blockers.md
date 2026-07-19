# Blockers — cold-rework

_Things needing human input. **None open** — the three that gated P3/P4 are resolved; the decisions are
in [`forks.md`](forks.md)._

Resolved:
- BK1 · composition machinery share vs duplicate → **F1** (shared `tick_pipeline!` macro).
- BK2 · cold-entity minting + `state` routing → **F2** (master-assigned `server_reference`; edge
  routes via `index.cold_shards`).
- BK3 · `route_reference` wildcard → **F3** (dropped — explicit region rows, no wildcard).

P1–P4 are all unblocked. New blockers get appended here as they surface.
