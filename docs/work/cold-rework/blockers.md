# Blockers — cold-rework

_Things needing your input before the phase they gate can execute. P1 (subtype fix) and P2 (router) are
**not** blocked — I can build those now. These gate **P3/P4** (the overlay + mutation). Newest-first._

---

## BK1 · How is the `state_log`/`state` composition machinery shared? (gates P3, P4)

**What.** "Cold rides the same machinery" needs the `clock` + `state_log` + `state` tables **and** the
`claim`/`write`/`promote`/`bump`/`gc` reducers on the `tile` and `thing` modules. Today that machinery
is **bespoke, hand-written in `data_shard`** — there is no shared engine (the `decl_tick_pipeline!` idea
from pipeline-generalization is not landed; `data_shard` uses shared codec helpers but its reducers are
local). So every module that composes duplicates ~200 lines.

**Options.**
1. **Duplicate** `data_shard`'s composition into `tile`/`thing` (copy the tables + reducers). Fast to
   P3; three near-identical copies to keep in lockstep (a real drift hazard — subscription SQL is
   string-typed).
2. **Factor first** — lift the composition into a shared crate or a `decl_tick_pipeline!`-style macro
   that `data_shard`, `tile`, `thing` all invoke (baseline tables + mint + fold are the per-module
   extras). One source of truth; a bigger up-front change that also touches the live `data_shard`.

**Why it blocks.** P3 adds these tables/reducers to the cold modules — I need to know whether to copy
or factor before writing them, because factoring reshapes `data_shard` too.

**Recommendation.** (2) — factor the composition into a shared module now, while there are only two
consumers, so cold is a thin include. It's the pipeline-generalization direction you already set. But it
means a `data_shard` refactor lands as part of P3; confirm you want that coupling, or prefer (1) to keep
P3 isolated and factor later.

---

## BK2 · Cold-entity id minting + `state` routing (gates P4)

**What.** A mutated cold cell needs an `entity_reference` (`server_reference:8 | object_reference:24`).
The mint pattern exists — `event_shard` uses a compile-time `const SERVER_REFERENCE =
pack_server_reference(TYPE_EVENT, 0)` + a counter. So each cold shard would mint with its own const:
`tile → (TYPE_BIOME_TILE, 0)`, `thing → (TYPE_BIOME_THING, 0)` — a mutated tree's `entity_reference`
then has `type_id = TYPE_BIOME_THING`. Two open pieces:

1. **Server-id allocation.** The const `server_id` is `0` per family today; a second shard of a family
   needs `server_id = 1`. Hardcoded const, or master-assigned like the orchestrator? (event_shard
   hardcodes; that may not scale.)
2. **Edge routing by `type_id`.** The edge must send an entity's `state` to the shard its `type_id`
   selects — but the edge is **single-instance with hardcoded DB names today** (no `type_id → shard`
   map). P4 needs that routing (it dovetails with P2's `cold_shards`).

**Why it blocks.** P4's mint step needs the server_reference decided, and the composed `state` can't
reach the client without the type_id routing.

**Recommendation.** Hardcode the family `server_reference` const for now (matches `event_shard`, one
shard per family), and extend P2's `cold_shards` lookup to also answer `type_id → shard` for state
routing. Confirm, or say if server-ids should be master-assigned from the start.

---

## BK3 · `route_reference` wildcard encoding (gates P2)

**What.** `cold_shards` routes `(type_id, region_reference) → shard`. The default ("all regions of a
family → shard 0") shouldn't need 256 rows. How is a wildcard region encoded in `route_reference`?

**Options.** (a) a sentinel `region_reference` (e.g. `0xFF`) meaning "any," matched after an exact
region miss; (b) a separate `default_shard` per type (a tiny second table or a column); (c) no wildcard
in the table — the **edge** falls back to a configured default shard when the lookup misses.

**Why it blocks.** P2 seeds the default route in `init`; the encoding decides that row's shape.

**Recommendation.** (a) — a `0xFF` "any-region" sentinel, exact-match-wins. One table, one default row
per family, no edge-side special case. Confirm or pick another.
