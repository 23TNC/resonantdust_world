# Forks — cold-rework

_Decision points, options, what we chose, why. Chronological._

---

## F1 · Composition machinery: **shared macro** (was BK1)

**Decision.** Factor `data_shard`'s bespoke composition into a shared `tick_pipeline!` macro that
`data_shard`, `tile`, and `thing` all invoke — not duplicate it three times.

**What moves:** the `clock` / `state_log` / `state` tables and `init` / `bump` / `claim` / `write` /
`gc` reducers (everything but the payload). `data_shard` becomes the payload struct (`TargetState` =
`definition_reference | position_reference | data`) + one macro invocation (~242 → ~15 lines). Cold's
payload is the **same** three-ref shape, so `tile`/`thing` invoke the same macro and add only their
baseline + seed + mint + fold.

**Why.** One source of truth for the composition (subscription SQL is string-typed — three drifting
copies is a live-bug hazard). It's the pipeline-generalization direction already set.

**Care.** This re-touches the **live** `data_shard`. The macro must emit byte-identical table shapes
and reducer signatures. **P3 step 1** = extract the macro and prove `data_shard` is unchanged (bindings
diff clean + the wolf still moves) *before* the cold modules build on it. Macro crate location settled
at build (a lightweight `#[macro_export]` crate the modules path-dep; it emits `spacetimedb::` tokens,
so it needn't depend on spacetimedb itself).

---

## F2 · `server_reference` is master-assigned; the edge routes via `index` (was BK2)

**Decision.** (a) A minting shard's `server_reference` is **master-assigned at standup** (a `server`
table set like `set_orchestrator`), not a compile-time const — the mint reads it. (b) The edge locates
shards through `index.cold_shards` for **both** the baseline subscription and the `state` routing — one
region-keyed router.

**Why.** Consistency with the rest of the standup (orchestrator is already master-assigned); the mint
needs a unique `server_reference` per shard and the master owns that allocation. One router avoids two
mechanisms.

**Consequence.** `event_shard`'s hardcoded `const SERVER_REFERENCE` becomes the outlier — a small
follow-up migrates it to the master-assigned pattern (not in this work's critical path). Only *minting*
shards carry the `server` table (`data_shard` doesn't mint), so it's a mint-side addition, not part of
the composition macro (F1).

---

## F3 · No `route_reference` wildcard — explicit region assignment (was BK3)

**Decision.** `cold_shards` holds **explicit `(type_id, region_reference) → shard` rows**; the master
assigns them as regions come online. No wildcard / sentinel region.

**Why.** The wildcard only existed to avoid seeding a row per region for the single-shard default — but
the world uses only a handful of regions today (around the origin), so we just seed those. An unassigned
region has no row until the master allocates it. Simpler and truthful; `route_reference` needs no
special encoding.
