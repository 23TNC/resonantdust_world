# Todo — spacetime rewrite (planned, not started)

Executes: the `shard` component. Planned work not yet begun; moves to [remaining.md](remaining.md)
when started. Newest-first.

**The rewrite is done.** Behavioral core + integration; the reference-model re-cut
(identity/event/object); the geographic geometry (legacy `zone_id`/`surface` retired); the
geographic cold `entity_reference`; PACK as an enqueued execute op (divergence #2 closed); and the
cross-shard foreign Phase-1 hold — all landed + verified ([completed.md](completed.md)). No blockers
open. One deliberate non-item remains:

## Deliberately NOT done — `cold` table `region_zone:u16` key (#4 remnant)

- **2026-07-14** · **RECOMMENDATION: leave as-is until multi-realm sharding is real.**
  `cold` keys by the **geographic** `zone_id:u32` (`realm|region|zone`), which is *correct* — the
  geometry re-cut already retired the legacy flat/surface layout. The design's `region_zone:u16`
  (realm implied by the shard) is a **wire-compaction**: it saves ~2 bytes per cold row.
  **Cost today:** the wire (`ColdObjectsRow`) is demuxed by the client on `zone_id`, and a client
  may hold subscriptions on **several shards** (several realms) over one edge connection — a row
  carrying only `region_zone` can't be attributed to a realm by the client, so the **edge** would
  have to track each shard's realm and reconstruct `zone_id` on every relayed row. That's new
  plumbing + a new failure mode for 2 bytes, with **zero payoff in a single-realm world** (realm 0).
  **Revisit when:** multi-realm sharding lands (the realm is then genuinely redundant per row and
  the edge already needs per-shard realm knowledge for routing).

_(This is a judgement, not a blocker — if you'd rather have the design's exact key width now, it's a
contained change: `Cold.zone_id → region_zone`, `seed_cold_row`/`mint_cold`/`find_or_mint`, the edge
subscribe filter + `ColdObjectsRow` reconstruction, and a shard republish.)_
