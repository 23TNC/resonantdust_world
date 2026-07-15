# S7 — Cold convergence + geographic keying + `hot_reference`

**Goal:** the deferred reconciliations, now safe to do off the critical DSL path. Closes
**divergences #3, #4, #5, #7**.

**Status:** ✅ **DONE (2026-07-14)** — #3, #4, #5, #7 and #10 all closed. (#4 closed **fully**: the
`macro_position:u16` row header was *not* a "key-width compaction" to defer — omitting it was the
D-3 live bug; see #11. #3 turned out to be untracked build detritus, not modules.) Only **#8** —
the deferred event/data shard split — remains open anywhere in this component. Live state:
[`work/spacetime-rewrite/`](../../../../../../../work/spacetime-rewrite/completed.md). The
object-model shard-class decisions this depended on are settled (blockers B-1 + B-2 both resolved).

## Changes (each independently landable)

- **#3 · Retire standalone cold modules.** ✅ **DONE (T-7).** Delete `cold_things` / `cold_tiles`
  ([`server/spacetime/server/modules/`](../../../../../../../../server/spacetime/server/modules)) once the
  in-macro `cold` table carries their data end-to-end (edge seeds it; client decodes it).
- **#4 · Geographic cold key.** ✅ **DONE.** The legacy flat `zone_id`
  (`region_x:8|region_y:8|surface:8|…`) was **retired**; `zone_id` is now geographic
  `realm:8|region:8|zone:8|reserved:8`. The `cold` row then dropped `zone_id` entirely for the
  design header — `macro_position_reference:u16` + `type_reference:u16` + `layer_id:u4`, keyed on
  their composite `cold_row_reference:u64` (**not** a key-width compaction but a missing header —
  divergence **#11**, closed 2026-07-14). The edge subscribes `WHERE macro_position = …` and
  reconstructs the client-facing `zone_id` from its shard's realm + `macro_position` when relaying.
- **#5 · Re-key hot identity + location index.** ✅ **DONE.** `state`/`state_log` keep a **`u64
  entity_key`** — it now holds the reference model's `entity_reference` (`reference_id:6 |
  server_reference:16 | object_reference:32`), so a hot object *is* `hot_reference:32`
  **server-qualified**; narrowing the column to a bare `u32` would drop the server qualification
  cross-shard identity + `home_shard` routing need (rationale:
  [work/…/forks.md](../../../../../../../work/spacetime-rewrite/forks.md)). The **location index**
  need is satisfied structurally: a cold object's `entity_key` **is** its location (`REF_COLD |
  server_reference | cold_reference(region|zone|tile|layer)`), so `find-or-mint`'s
  "is there a hot entity at this location?" is a **primary-key lookup on `state`** — no separate
  index required.
- **#8 · Event-shard / data-shard split.** Deploy the `event_log`+lifecycle and the
  data tables (`state`/`cold`/holder) as separate shard roles of the one generic module
  ([lifecycle.md](../../intent/lifecycle.md)). Deployment/routing work, not a code fork.
- **#10 · Geographic `server_reference`.** ✅ **DONE.** `server_reference = realm_id:8 | server_id:8`
  (geographic). The "where does `server_type` (routing) live" fork is **moot**: the functional
  machinery (`server_type` / `server_ref_is_db` / `action_reference` / the `zone_reference:64`
  aggregate) had **zero live consumers** — the worker maps a `server_reference` to a DB connection
  via its `SHARDS` env list, not a type tag — so it was **deleted**, not relocated.

## Verify

Per item: a zone renders from the converged `cold` table in the browser; routing still works
after the key change; hot entities keep stable identity through a re-key.

**Result (2026-07-14):** all three verified — the browser renders the forest + wolves from the
converged `cold`/`state` tables on the re-cut schema; zone routing works after the geographic
`zone_id` repartition; hot entities keep stable identity (`entity_key = REF_HOT | server | hot_ref`)
across the re-key. Plus the cold round-trip (Interact → find-or-mint → PACK settle) verified.

## Why last

Each of these drags the reference/shard redesign, which is why they were kept out of S0–S6 — the
DSL lands first on the existing identity/keying, then these tidy the foundation underneath it.
