# Todo — spacetime rewrite (planned, not started)

Executes: the `shard` component. Planned work not yet begun; moves to [remaining.md](remaining.md)
when started. Newest-first.

The whole reference-model re-cut is **done + verified** ([completed.md](completed.md)): the
hot/identity/event side, the object-model side (`object.rs`), the geographic geometry (G1 — legacy
`zone_id`/`surface` retired → realm/region/zone/tile/layer), the geographic cold `entity_reference`
(G2), and the PACK trigger (G3). No blockers open. What's left is marginal / deferrable:

## Marginal representation follow-ups (unblocked, low value)

- **2026-07-14** · **`cold` table `region_zone:u16` key (#4)** — the `cold` table keys by the
  (now-geographic) `zone_id:u32`, which is correct. Narrowing to `region_zone:u16` (realm implied by
  the shard) saves ~2 bytes/row but needs the edge to carry each shard's realm to reconstruct the
  full `zone_id` for the client wire (`ColdObjectsRow` is demuxed by `zone_id`). No dev payoff
  (single realm 0); do it if/when multi-realm sharding lands.

## Not blocked (small, deferrable)

- **2026-07-14** · **Cross-shard foreign Phase-1 hold** — a foreign target gets no pending
  row/holder on its home shard during the in-flight window (read-rule/GC visibility). The
  convergent *write* is done ([completed.md](completed.md) #4); this is the in-flight *hold*.
  Eventually-consistent today; low priority.
