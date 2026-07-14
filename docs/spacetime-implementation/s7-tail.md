# S7 — Cold convergence + geographic keying + `hot_reference`

**Goal:** the deferred reconciliations, now safe to do off the critical DSL path. Closes
**divergences #3, #4, #5, #7**.

**Status:** ⬜ not started. **Depends on:** [S6](s6-hot-cold.md) and the object-model
shard-class decisions.

## Changes (each independently landable)

- **#3 · Retire standalone cold modules.** Delete `cold_things` / `cold_tiles`
  ([`server/spacetime/server/modules/`](../../server/spacetime/server/modules)) once the
  in-macro `cold` table carries their data end-to-end (edge seeds it; client decodes it).
- **#4 · Geographic cold key.** Move `cold` from the flat `zone_id` key to `region_zone`
  ([../references/spatial-references.md](../references/spatial-references.md)); reconcile against
  the legacy flat `zone_id` routing.
- **#5 · Re-key hot identity + location index.** Close D3's deferral — re-key `state`/`state_log`
  to `hot_reference:u32` (+ `server_reference`) if we take that path
  ([../references/hot-cold-references.md](../references/hot-cold-references.md)); otherwise
  document why `u64 entity_key` stays. Either way, add the **location index** on `state`
  (realm·region·zone·position·layer·type_id) that enqueue's `find-or-mint` needs
  ([S3](s3-worker.md)).
- **#8 · Event-shard / data-shard split.** Deploy the `event_log`+lifecycle and the
  data tables (`state`/`cold`/holder) as separate shard roles of the one generic module
  ([lifecycle.md](../spacetime-tables/lifecycle.md)). Deployment/routing work, not a code fork.
- **#10 · Geographic `server_reference`.** Settle `server_reference = realm_reference:u8 +
  server_id:u8` vs. the functional `server_type:6 | server_id:10`, with the object-model
  shard-class work. Decide where `server_type` (routing) lives (the open fork:
  [../references/reference-vs-id.md](../references/reference-vs-id.md)).

## Verify

Per item: a zone renders from the converged `cold` table in the browser; routing still works
after the key change; hot entities keep stable identity through a re-key.

## Why last

Each of these drags the reference/shard redesign, which is why they were kept out of S0–S6 — the
DSL lands first on the existing identity/keying, then these tidy the foundation underneath it.
