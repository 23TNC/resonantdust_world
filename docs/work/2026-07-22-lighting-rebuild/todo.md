# Todo — lighting rebuild (execution order)

_Planned, not started. Phases run in order; each is verifiable on `/overlayRT shadow-cold`. Items
move to [`completed.md`](completed.md) when done **and** verified. See [`README.md`](README.md) for
the model + constants, [`forks.md`](forks.md) for decisions, [`issues.md`](issues.md) for the
failures we're rebuilding away from._

---

_**P0–P4 done + verified** → [`completed.md`](completed.md) (P0–P2 2026-07-22; P1.5 + P3 + P4
2026-07-23 — silhouette shadows at zoom 0.5/1/2). Remaining P0 item: **archive the
`2026-07-22-shadow-corridor` stream** out of the repo — deferred to the end so cross-links don't
break mid-rebuild._

## P6 · Rebuild the corridor (pure optimization)

- [ ] Replace the brute-force reach-walk with a corridor that yields **identical output**.
- [ ] Acceptance: output **matches the brute-force baseline** (diff the shadow map). Must NOT
      reproduce the previous corridor's failures ([`issues.md`](issues.md)): wedges, 3-tile cap,
      out-of-corridor pickups.
- [ ] Commit P2–P5 (functional + brute force) **before** starting P6.
