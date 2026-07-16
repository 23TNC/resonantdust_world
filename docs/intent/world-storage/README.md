# world-storage — a generic shard family for the world's objects

> **Status: PLAN / intent.** Nothing built. Shapes cite [`VARIABLES.md`](../../VARIABLES.md) (the
> object model — `cold_row_reference`, `kind_pos_reference`, `type_id`) and
> [`TABLES.md`](../../TABLES.md). Builds on the live sim core (`intent/spacetime-again/`). Authored
> 2026-07-16.

**What this is for.** Store every kind of world object — terrain tiles, scattered things, pawns,
players, items — across many shards, **sharing one storage/composition machinery** while each object
type defines its **own payload**. Instead of a bespoke module per type, we stamp out a module per
type from one generic core and a payload definition.

**The one idea.** A shard is `(row-key, payload)` + a zone-keyed client-visible projection. The
*machinery* (the tables, the zone subscription, the write path, and — for movers — the tic
composition pipeline) is generic; the *payload* is the type parameter. Define a payload, get a module.

---

## Two regimes: hot and cold

The split is **how often it changes**, which decides whether it needs the tic pipeline:

| | **hot** (movers) | **cold** (terrain) |
|---|---|---|
| examples | pawns, players | biome-tiles, biome-things |
| row key (u32, realm-unique) | `entity_reference` = `server_reference:8 \| object_reference:24` | `cold_row_reference` = `macro_position:16 \| layer_reference:8 \| server_reference:8` |
| slot uid (u64) | `reserved:16 \| entity_reference:32 \| tic:16` | `reserved:16 \| cold_row_reference:32 \| tic:16` |
| granularity | one **entity** per row | one **zone-layer** per row (a Vec of members) |
| changes | every tic (movement) | rarely (worldgen seed; an occasional edit) |
| write path | the composition pipeline (orchestrator → worker → absolute finals), promoted to `state` | worldgen bulk-seeds; an edit — see §Open |
| history | per-`(entity, tic)` slots (`state_log`) | current-value; per-tic slots only if edits run the pipeline |

**The key is realm-unique in a `u32`** (realm is the shard's, never per reference), so the slot uid is
the **same `u64` for both** — `reserved:16 \| key:32 \| tic:16`, `u16` reserved to spare. The
composition core treats the key as an **opaque realm-unique u32**; only its *interpretation* differs.
So `state_log` is one shape parameterized by payload alone. (This replaces the old `u64`
`cold_row_reference` in [`VARIABLES.md`](../../VARIABLES.md) — the `u16 type_reference` was the fat
part; `layer_reference:8` + `server_reference:8` carries the same in half the bits. **Consequence:**
the row no longer carries `subtype_id:12` — if a cold row ever needs subtype, it moves to the per-entry
`kind_reference` or the layer. Confirm when VARIABLES is revised.)

They **share**: the payload-generic storage, the identical `state_log` slot, the **zone-keyed client
subscription** (`WHERE macro_position_reference = <zone>`), and the module skeleton. They **differ**
only in the write path — hot runs the pipeline (built: `spacetime-again`); cold's is §Open.

---

## The payloads (per type — all cite [`VARIABLES.md`](../../VARIABLES.md) §Cold storage)

- **biome-tile** (cold, dense): payload `Vec<u16>` — 256 `kind_reference`s, one per tile, indexed by
  `tile_reference` (0..256 = the 16×16 zone). **No** per-entry `tile_reference` (it's the index) and
  **no** `data` — a tile is only *what kind*. (Stretch: `Vec<u8>` if kinds stay < 256.)
- **biome-thing** (cold, sparse): payload `Vec<u32>` — `kind_pos_reference` per member
  (`kind_reference:16 \| tile_reference:8 \| data:8`), because a thing carries *where in the zone* and
  *its data* (facing/count). Sparse: only occupied tiles.
- **pawn** (hot): payload = the three orthogonal references already live in `data_shard`
  (`definition_reference:u32 \| position_reference:u32 \| data:u8`). A per-pawn definition (sprite)
  wants a definition verb — the `spacetime-again` deferred item.
- **player**, **item**, … : new modules, new payloads, later.

Row header `(macro_position_reference, type_reference, layer_id)` reconstructs each member's full
identity: `definition_reference` = row `type_reference` + entry `kind_reference`; `position_reference`
= row `macro_position` + `layer_id` + `type_id` + entry `tile_reference` (VARIABLES §Cold storage).

---

## The generic core

A declarative macro — `decl_shard!` — stamps a module's tables + reducers from `(regime, payload)`,
the way `spacetime-again`'s pipeline generalized over payloads ([[pipeline-generalization]] in memory).
It lives in a shared crate (`shared/shard`?) bind-mounted like `shared/codec`, so every module is a
thin instantiation:

```
// server/spacetime/server/modules/biome_tile/src/lib.rs
decl_shard!(cold, payload = Vec<u16>);   // → cold table (zone-keyed), seed + patch reducers, subs

// modules/biome_thing/src/lib.rs
decl_shard!(cold, payload = Vec<u32>);

// modules/pawn/src/lib.rs  (today's data_shard, re-expressed)
decl_shard!(hot,  payload = Pawn);       // → state_log + state, claim/write/gc — unchanged behavior
```

`cold` expands to: a public `cold` table `{ cold_row_reference PK, macro_position_reference idx,
type_reference, layer_id, members: Vec<P> }` + `cold_removed` tombstones (a `tile_reference:u8`, per
VARIABLES) + `seed`/`patch` reducers + the zone subscription. `hot` expands to today's
`state_log`/`state` + `claim`/`write`/`gc`. Both are payload-parametric.

---

## Phasing (one module at a time — the method that carried `spacetime-again`)

1. **Extract the core.** Lift `data_shard`'s machinery into `shared/shard` behind `decl_shard!(hot,
   …)` and re-instantiate the pawn module from it — **zero behavior change**, proven by re-running the
   live W3/W5 checks. This is the generalization proof (one macro, the existing module falls out).
2. **`cold` regime + biome-tile module** (`Vec<u16>`). Worldgen seeds it; the edge subscribes the
   zone's cold rows; the client renders the ground. This **revives the deferred terrain path** —
   `edge/index.rs` (zone→shard routing) and `worldgen.rs` return here, and `Event::ColdObjects`
   (kept in `client/core` for exactly this) finally fires.
3. **biome-thing module** (`Vec<u32>`). Sparse scatter; client renders things over the ground.
4. **pawn definition** — plumb `definition_reference` so pawns render their real sprite (ties to
   `spacetime-again`'s deferred `CREATE`/definition verb), not the fallback thing.
5. **more types** (player, items) as new `decl_shard!` instantiations, as gameplay needs them.

---

## Open — decisions to make before Phase 1

1. **Cold edits: direct or through the pipeline?** Worldgen seeds cold directly regardless. But an
   *edit* (chop a tree → change a `biome-thing` member) happens at a tic. **(a)** Direct: an edit
   reducer read-modify-writes the row's Vec, out of band — simplest, but cold edits get no ordering
   vs hot events and no worker isolation. **(b)** Through the pipeline: a cold row is an "entity", an
   edit is an event a worker composes (read prev Vec → apply → write absolute Vec), so *all* mutation
   shares one ordered path. (b) is the cleanest "share the machinery" but makes the composition core
   handle a `u64` row key + a Vec payload (read-modify-write, not overwrite). **Leaning (b)** for
   uniformity; (a) is fine if cold edits stay rare and unordered-with-hot is acceptable. **Your call.**
2. **Generalization mechanism:** `decl_shard!` macro (recommended — type-safe, zero-cost, "a bunch of
   modules" is literal) vs a raw-bytes payload column each module reinterprets (fewer modules, loses
   the typed schema) vs one enum payload (one module, all types coupled). Recommend the macro.
3. ~~Slot key width for a cold composition.~~ **Dissolved** — the cold row key is `u32`
   (`macro_position:16 | layer_reference:8 | server_reference:8`), realm-unique, so `(key, tic)` is
   the *same* `u64` `state_uid` as hot (`reserved:16 | key:32 | tic:16`). No wider uid; the machinery
   is literally one shape. Requires revising `cold_row_reference` `u64 → u32` in VARIABLES.
4. **Where the core crate lives:** `shared/shard` (new, bind-mounted like `shared/codec`) vs folding
   into `shared/codec`. Recommend a dedicated crate — it pulls `spacetimedb`, which `codec` doesn't.
5. **One module per type, or grouped by regime?** e.g. all cold types in one module keyed by
   `type_reference`, vs a module per type. Per-type modules (your suggestion) route + scale
   independently and keep payloads un-coupled; grouping is fewer DBs. Recommend per-type.
