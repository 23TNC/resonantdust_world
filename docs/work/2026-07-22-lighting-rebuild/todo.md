# Todo — lighting rebuild (execution order)

_Planned, not started. Phases run in order; each is verifiable on `/overlayRT shadow-cold`. Items
move to [`completed.md`](completed.md) when done **and** verified. See [`README.md`](README.md) for
the model + constants, [`forks.md`](forks.md) for decisions, [`issues.md`](issues.md) for the
failures we're rebuilding away from._

---

_**ALL PHASES COMPLETE + VERIFIED 2026-07-23** → [`completed.md`](completed.md): P0–P2 (strip /
data textures / geometric quads, 2026-07-22), P1.5 + P3 + P4 (shared grid + opaque bbox +
silhouette shadows), P5 (scoped dirty routing), P6 (corridor bit-identical to brute force), and
the deferred P0 archive (`2026-07-22-shadow-corridor` → `~/archive/shadow-corridor/`). Open
follow-ups live in [`forks.md`](forks.md): F1 (LOD reconciliation), F2/C5 (multi-page
silhouettes)._
