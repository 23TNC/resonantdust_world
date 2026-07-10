# Generalizing the tick pipeline — one engine, any data structure

**Status:** Design (2026-07-10), branch `0.2.1`. Extends [`simulation.md`](simulation.md)
(the tick engine) and [`object-shard.md`](object-shard.md) (the transfer saga), and
**supersedes** the cold-blob-first plan in [`world-on-pipeline.md`](world-on-pipeline.md)
(see [Zones as a pipeline](#zones-as-a-pipeline--the-u64-256-payload)). Nothing built
yet; this is the target the `decl_tick_pipeline!` macro (already promised in
[`pipeline/src/lib.rs`](../spacetime/server/pipeline/src/lib.rs) line 9) builds toward.

## Why

The tick engine's *scheduling spine* — tic model, work-gen, claim/lease fence, read
rule, priority-DAG, GC horizon — never inspects an entity's payload. It only ever
**copies** the game fields (`kind`/`zone_id`/`location`/`rotation`/`offset`/`data0`/
`data1`) verbatim and reasons purely over ids and control fields. So the same engine can
carry *any* payload; today it just hard-codes the one spatial payload that objects and
zones happen to share.

Generalizing lets one `server_worker` pool resolve **any** pipeline — pawns, objects,
tiles, zones, and whatever non-spatial structures come later (inventories, job queues,
markets) — because all a worker touches is `actor`/`target` (both `u64`), the actor's
server (`u16`), and an `action` (`u16`), all of which become generic references. New
data structures cost a payload definition plus a set of `action_id` handlers; the engine
is untouched.

## Three layers

Every field in the engine sorts into exactly one band. The generalization is: **freeze
the spine, keep routing keys as typed columns, parameterize the payload.**

| Layer | Fields | Disposition |
|---|---|---|
| **Control spine** | `event_reference`, `sequence`, `event_tic`, `tic`, `dirty`, `status`, `server_id` (fence), `created_at`/`assigned_at`, `master_tic`, `actor_key`/`target_key`, the provenance set | Fixed, identical for every pipeline; the engine reasons only over these |
| **Routing keys** | `zone_id` today (per-pipeline) | Stay top-level **indexed** columns — the edge subscribes `WHERE zone_id`; a filterable field can't live inside an opaque payload |
| **Payload** | `kind`, `location`, `rotation`, `offset`, `data0/1` today | Parameterized per pipeline (`decl_tick_pipeline!`) |

The whole effort is moving fields from column three out of the hard-coded schema and
into a macro parameter, while the reference layouts below make the spine's ids generic
enough to name anything.

---

## Reference layouts

All three live in [`shared/codec`](../shared/codec/src/packed.rs) — the one crate
bind-mounted into the SpacetimeDB modules **and** linked by `server`/`server_worker`/
client, so every process packs and unpacks with identical code (as `entity_key` already
does today). Define the layouts and their `pack_*`/`unpack_*` helpers there once; changing
a layout is a one-file edit that propagates everywhere.

### `server_reference` — `u16 = server_type:6 | server_id:10`

Names any process or database in the system, globally, coordination-free: the
`(server_type, server_id)` pair is unique because each type hands out its own `server_id`
range. Replaces the bare `u16` server ids.

- **`server_type:6`** — up to **64** kinds of server. Today: `master`, `worker`
  (renamed from `simulation`), `edge`, and the database classes (`object-db`,
  `zone-db`, `tile-db`, …). ~10 in use. **This is the tightest cap** and the hardest to
  widen later (baked into every reference); hand out type values with gaps/reserved
  ranges, don't pack them densely.
- **`server_id:10`** — up to **1024** instances of a given type. Far past what we'd
  operate.
- **Type *ranges* mark which server_types are DB-backed** (have a `DbConnection`). A
  worker resolving a cross-shard read asserts `actor_server_reference`'s type is in the
  database range before mapping it → connection, and rejects a malformed reference at the
  door. The type field is both a router and a guard.

### `entity_reference` — `u64`, `entity_type`-discriminated

Names any simulation entity in one comparable space. **`entity_type:8` (top byte)
discriminates the layout of the remaining 56 bits** — generalizing the current 2-bit
`ENTITY_TAG`. This is load-bearing: it lets *minted* and *positional* entities coexist in
one `u64`, which is what keeps `priority = hash(tic, entity_reference)` a **single total
order across all classes** so cross-class dependency cycles (pawn ↔ trap tile) break
deterministically. Do not fragment into per-pipeline key widths — interacting pipelines
must share this key space.

Two families, chosen by `entity_type`:

- **Minted** (pawns, objects, tiles) — entities allocated an id by the server that
  created them:
  ```
  entity_type:8 | entity_id:32 | mint_server_reference:16 | (8 reserved)
  ```
  Unique by construction: the minting `server_reference` is globally unique and permanent,
  `entity_id` is that server's monotonic per-server counter, so no two servers — and no
  server twice — collide. (This is today's `object_reference = obj_type | shard | count`
  promoted to the universal form, with `mint_server_reference` generalizing `shard_id`.)
- **Positional** (zone cells) — entities whose identity *is* their location, never
  minted (no allocation, no lookup; idle cells cost zero rows — the property that makes
  static terrain free):
  ```
  entity_type:8 | zone_id:32 | location:8 | layer:8 | (8 reserved)
  ```
  Unique because `zone_id` is world-global. There is no `mint_server_reference` — a
  positional entity has no minter.

`entity_id:32` gives ~4.3 B lifetime mints per server (deaths don't free it — a hard but
generous per-server ceiling, same as today's object count).

### `action_reference` — `u16 = data_type:6 | action_id:10`

Namespaces actions per pipeline so one `server_worker` can resolve any of them by
dispatching on `data_type`.

- **`data_type:6`** — up to **64** pipeline types (≈ `Domain` implementations). Today ~3:
  pawn, object, zone. **Watch this cap** — it's baked into both `action_reference` and the
  `entity_type → data_type` map; leave reserved gaps.
- **`action_id:10`** — up to **1024** actions per pipeline. Plenty.
- **Reserve one `data_type` (or a low `action_id` band) for shared/universal actions** —
  the saga verbs (`transfer`/`store`/`check_stored`, i.e. today's `receive`/`ack`) and any
  cross-class action (`damage` targeting an entity in another pipeline). Cross-class
  actions target an entity in a *different* pipeline than the actor; without a shared
  namespace every pipeline would re-declare them. These verbs are already generic in
  [`shared/tick/src/domain.rs`](../shared/tick/src/domain.rs), so a shared namespace
  matches the code.

### `entity_type → data_type` map

`entity_type` (256) is wider than `data_type` (64): the map is **many-to-one** (e.g. a
`tile` and a `zone-cell` both resolve on the zone `Domain`). Define it explicitly in
`shared/codec`, because the worker dispatches which `Domain`/`apply_event` to run **by
it** — and a cross-entity actor read must resolve the *actor's* `Domain` from the
*actor's* `entity_type`, not the target's. Without the map the worker can't pick the right
resolver for a cross-class read.

---

## The generic engine

The four+ tables and their reducers, parameterized. `server_id` here is the fence token
(a `server_reference` of a worker); all keys are `entity_reference`s.

### `event_log` — intent + provenance

Expanded from today to carry provenance, so **worker-generated events are safe without a
deterministic dedup key** (see [Provenance](#provenance-validated-events)). Fields, with
the renames and additions from the design dialogue:

```
u64  event_reference          // minted global identity: mint_server_reference:16 | reserved:16 | event_id:48
u64  sequence                 // per-shard auto_inc — the ONLY composition order (see note)
u32  event_tic
u16  source_server_reference  // who CREATED the event (was from_server_id)
u16  actor_server_reference   // where actor_key lives — the cross-shard read target (was actor_shard_id)
u16  requesting_server_reference // who requested it (need not be the source)
u16  worker_server_reference  // the worker that generated/handled this event's trigger
u64  trigger_event_reference  // the event that generated this one, if any (0 = none)
u64  actor_key                // entity_reference
u64  target_key               // entity_reference
u16  action                   // action_reference
u64  data0, data1
u8   status
```

Two deliberate splits vs. the naive version:

- **Identity vs. order.** `event_reference` is **minted** (global, so
  `trigger_event_reference` can point cross-shard) but minting destroys the per-shard
  monotonicity that ordered composition. So composition order moves to a **separate
  per-shard `sequence: auto_inc`** — ST serializes reducer appends, giving a well-defined
  commit order. Invariant #5 ("ordered by per-shard reference") now reads *by
  `sequence`*, never by the minted `event_reference`.
- **`event_id:48`, not 32.** Events mint every action every tic — churn orders of
  magnitude above object counters. u32 wraps in weeks at a few k events/sec; the 48-bit
  field (spending the reserved bytes) pushes wrap past any real horizon.

`resolve` **stamps `worker_server_reference` onto the consumed events** as it marks them
complete — that recorded winner is what provenance validation reads.

> **TODO (open):** which reference carries the *home shard* of a trigger event. A minted
> `event_reference`'s `mint_server_reference` is E's *creator*, but E lives on its
> *target's* shard — those differ. Validation must locate E to read its recorded winner,
> so one field must carry E's home shard. `source_server_reference` is defined as
> "creator," so it can't double as "home shard." Resolve before locking the schema.

### `state_log` / `state` — payload parameterized

Identity and control (`entity_key`, `tic`, `dirty`, `server_id` fence, timestamps,
`status`; `state.tic` for staleness) are fixed. Everything the current schema lists after
them — `kind`, `zone_id`, `location`, `rotation`, `offset`, `data0`, `data1` — becomes the
**macro's payload parameter**, plus each pipeline's declared **routing-key** columns
(indexed, e.g. `zone_id`) that must stay filterable in SQL.

### `tic_meta`, mint counters

`tic_meta` (the master-tic row) is unchanged. Object-id minting
(`shard_meta`/`object_counter` + `mint_object_id`/`spawn_object`) generalizes to
**per-server entity minting** and lifts *out* of the core engine into the minting-server's
instantiation — the core keeps only `seed_entity` (explicit key) + the tick reducers. An
**event-id counter** joins it (for the minted `event_reference`).

### `decl_tick_pipeline!`

The vehicle. SpacetimeDB's `#[table]`/`#[reducer]` are attribute macros on **concrete**
structs (no generics), so a declarative `macro_rules!` that stamps out monomorphized
tables + reducers per (key, server-id, routing, payload) is the way — declarative-macro
expansion runs first, so the ST attributes apply to the emitted items. Confirmed feasible:
the module is already just `pub use resonantdust_pipeline::*`
([`shard/src/lib.rs`](../spacetime/server/modules/shard/src/lib.rs)), and re-export
registers tables fine.

```rust
decl_tick_pipeline! {
    key:       u64,                 // entity_reference
    server_id: u16,                 // server_reference
    routing:   { zone_id: u32 },    // indexed, SQL-filterable
    payload:   { kind: u16, location: u8, rotation: u8, offset: u8, data0: u64, data1: u64 },
}
```

Each shard module invokes it once with its own payload; interacting pipelines share the
`key`/`server_id` types.

---

## The worker — a generic `Domain`

The scheduling half of [`shared/tick`](../shared/tick/src) (`priority.rs`, `read_rule.rs`,
the fence, cross-shard routing) is **already generic over `entity_key: u64`** and needs no
change. Only [`domain.rs`](../shared/tick/src/domain.rs) — the `EntityState` struct + the
per-action `apply_event` — is payload-specific. Lift it behind a trait:

```rust
trait Domain {
    type Payload;                                   // the pipeline's game state
    fn apply_event(state: Payload, ev: &Event, actor: Option<&Payload>) -> Payload;
    fn routing(state: &Payload) -> RoutingKeys;     // e.g. zone_id, for the relay
}
```

Today's spatial payload becomes `SpatialDomain` (move/damage/receive/ack). The worker
binds `Domain` + the module's generated `StateLog`/`EventLog` types; `data_type` selects
the impl. `priority`/`read_rule`/the fold loop stay shared and untouched.

---

## Provenance-validated events

Workers may **append follow-on events** (the engine of worker-driven sagas). The fence
protects the `resolve` write but **cannot** protect a cross-shard event append (appends go
to another shard's `event_log`; writes are single-shard). So correctness comes from
validating provenance at **consume** time, not from preventing the duplicate append:

1. A worker resolving trigger `E` appends follow-on `F` **optimistically**, stamping
   `F.trigger_event_reference = E` and the generating worker.
2. `resolve` stamps `E.worker_server_reference` = the fence winner, atomically, as it
   completes `E`.
3. Before *any* worker acts on `F`, it looks up `E` (cross-shard, same subscription
   mechanism as an actor read) and honors `F` **iff `F`'s generating worker ==
   `E.worker_server_reference`** — else marks `F` dropped.

Because the fence yields exactly one winner for `E`, only that winner's follow-ons
validate; a losing/evicted worker's orphan follow-ons are dropped. This closes the
duplicate-append hole **and** makes the routing choice inside a saga **non-deterministic-
safe**: two workers may route the same tile to different zone shards, but only the
winner's `store` validates, so exactly one destination survives — the choice need not be a
deterministic function of position.

**The one ordering rule:** validate-before-effect. A worker must run the provenance check
as the *first* thing it does on a triggered event, so a soon-to-be-dropped `store` never
lands a partial effect. (This relaxes determinism for *side-effects* only; committed
*state* stays consistent regardless — immutable, single writer — you just give up
replay-reproducibility of the routing decision, which for placement is exactly what
doesn't matter. Keep genuine per-entity state math deterministic where cross-shard reads
depend on it.)

Provenance is generic (not pipeline-specific), so it lives in the engine core, and it
**closes three open items** in [`gaps.md`](gaps.md) #3: the pre-`receive` transfer-flag
hop (a `pack` puts the source in transfer-state before emitting follow-ons), the manual
cross-tic sequencing (the worker drives it), and the blind tombstone (verification, below).

---

## Pack / unpack as the migration model

Pack/unpack (hot ↔ cold world objects) is **the transfer saga generalized** — not a
bespoke cold-blob subsystem. This is what supersedes
[`world-on-pipeline.md`](world-on-pipeline.md): hot objects/tiles/things live in their own
pipelines/shards; **zones are their own shard/pipeline and are the cold store**; a settled
hot entity *packs into* a zone via ordinary events + cross-shard reads.

Worked example — a settled tile packing into a zone (server_worker = the renamed
`server_simulation`):

1. **`pack`** targets the tile on the tile shard (`actor = world`, system-injected). Its
   resolution puts the tile in a transfer-state and — the worker, knowing the tile's
   resolved position — determines the destination zone shard (needn't be deterministic;
   provenance guards it) and appends the two follow-ons:
2. **`store`** on the **zone** shard (`actor = tile`, cross-shard). Zone reads the settled
   tile, writes `cell[tile.location]` in its packed payload → the zone row goes `!dirty`.
   *(This is `receive`.)*
3. **`check_stored`** on the **tile** shard (`actor = zone`, cross-shard). Tile blocks on
   the zone being `!dirty` (the read rule), then **verifies** the tile's `kind` landed at
   the tile's `location` in the zone, and only then tombstones the hot tile. *(This is
   `ack`, hardened — verification, not a blind tombstone.)*

The ordering is deadlock-free by the existing read rule (`store` blocks on the tile
settling; `check_stored` blocks on the zone's write), and the direction is the safe one:
transiently the tile exists both hot and cold, **never neither** — a failed verification
leaves the tile hot to retry. `unpack` is the mirror (zone → hot tile shard). The only new
code is the `store`/`check_stored`/`pack` handlers in each `Domain` — no bespoke
machinery.

---

## Zones as a pipeline — the `[u64; 256]` payload

A zone is a **single fixed-width pipeline entity**, not a blob side-table and not 256
per-cell rows. Its payload packs every layer of all 256 cells into a fixed array:

```
cell: u64 = tile:9 | stack:17 | primary:14 | secondary:14 | tertiary:10
payload   = [u64; 256]  (2 KB)  + biome + frontier/version
```

- `tile:9` selects the floor; `stack:17 = count:5 | def:12` (up to 32 stacked); `primary`/
  `secondary` `= def:12 | rotation:2`; `tertiary` (utilities) `= def:8 | rotation:2`.
- One row, ~2 KB, one subscription per zone (the cold-blob's efficiency, without a
  side-table). ST holds it as a `Vec<u64>`/array column via `SpacetimeType`.
- It's the **first divergent payload** and the proof the parameterization is real: the
  zone payload (`[u64;256]+biome`) is nothing like the object payload (the little spatial
  struct). The `store` handler is one line — `cell[loc] = pack_cell(kind…)` — in the zone
  `Domain`.
- Objects written into a zone carry **no individual id and little dynamic data** — the
  trade that buys a huge amount of static/procedural content per zone cheaply.

Write amplification: a single-cell change rewrites the whole 2 KB entity, and
`check_stored` reads the full 2 KB cross-shard to verify one cell. Fine for
write-on-change static-ish zones; know it's there if a zone ever churns hard.

### Cell / kind codec (content layer, not engine)

The `kind` top-bit prefix code — `b1` tile, `b01` placeable (primary/secondary), `b001`
tertiary — and the biome-folding tile resolution live in `shared/codec` alongside the
reference packers, consumed by worldgen (assign) and the client (resolve → texture). It
does **not** touch the engine. Two inconsistencies from the sketch to reconcile when
writing it:

> **TODO:** biome width — the tile kind is `b1 | biome:8 | tile_bottom:7` (u8) but
> resolution reads a **u16** biome (`biome >> 8`, `biome & 0xFF`). Decide whether the
> per-zone biome is u16 and the kind's field is a *selector into* it.
>
> **TODO:** `primary` and `secondary` were both sketched as `b0100 | u12` — identical
> prefixes can't distinguish them; secondary needs its own (e.g. `b0101`).

The resolution intent (for the codec): `tile & 0x100` → default tile (`0x80 + tile&0xFF`,
255 defaults); else `tile & 0x80` → high-biome set (`biome >> 8`); else low-biome set
(`biome & 0xFF`) — two sets of 255 biomes, 128 tiles each, sharing the bottom 128 defaults
as a fallback when a biome-specific tile is absent.

---

## Consumers to parameterize

The payload is hand-listed in six places today; each becomes macro-emitted or
`Domain`-driven. From the consumer map:

| Consumer | File | What changes |
|---|---|---|
| Pipeline module | `pipeline/src/lib.rs` | Becomes `decl_tick_pipeline!` invocation(s) |
| Worker policy | `shared/tick/src/domain.rs` | `EntityState`/`apply_event` → `Domain` trait + impls |
| Worker | `server_simulation/` → **`server_worker/`** | `entity_state()` mapper + 10-arg `resolve()` call become `Domain`/macro-driven; gains provenance validation + follow-on append; renamed |
| Master | `server_master/` | Unaffected (clock only) |
| Edge relay | `server/src/ws.rs` | `state_row()` per-pipeline; keeps routing-key subscriptions |
| Rust client | `client/src/protocol.rs` | `StateRow` per-pipeline (decodes `entity_reference` via codec) |
| Bindings | per-crate `bindings/shard/` | Regenerate via `generate-bindings.sh` (dedupe into a shared crate is a companion cleanup, `gaps.md`) |

---

## Phased build

1. **Macro no-op refactor.** Wrap today's exact object/zone payload in
   `decl_tick_pipeline!`. Same tables, same reducer signatures, byte-identical bindings.
   Ship green before anything varies — if the macro reproduces the current schema exactly,
   the rest is changing its arguments.
2. **Generic `Domain`.** Extract the trait; move the spatial payload into `SpatialDomain`.
   `priority`/`read_rule`/fold untouched. Worker dispatches by `data_type`.
3. **Reference layouts + provenance.** Land `server_reference`/`entity_reference`/
   `action_reference` + packers in `shared/codec`; expand `event_log` (provenance fields,
   `sequence`/identity split, `event_id:48`); generalize minting to per-server;
   `resolve` stamps the winner; worker gains validate-on-consume + follow-on append.
   Rename `server_simulation → server_worker`.
4. **First divergent payload — the zone.** Instantiate the `[u64;256]` zone pipeline and
   the pack/unpack saga end-to-end; retire the cold-blob plan. This is the instance that
   proves the abstraction wasn't shaped to a sample size of one.

---

## Caps & constraints (keep an eye on)

- **64 server types** (`server_type:6`) — tightest, hardest to widen; reserve ranges.
- **64 pipeline types** (`data_type:6`) — ~3 today; reserve ranges, one for shared saga
  actions.
- **1024 servers per type**, **~4.3 B entities per server** (`entity_id:32`) — beyond
  operational reach.
- **Interacting pipelines must share the `entity_reference` key space and a comparable
  priority order** — do not per-pipeline the key width; the `entity_type`-discriminated
  layout is how divergent entities still share one `u64`.
- **Routing keys stay top-level indexed columns** — anything the edge/clients filter a
  subscription on can't live inside the opaque payload.

## Open items

- **Trigger home-shard field** (schema-blocking) — see the `event_log` TODO.
- **Biome width + primary/secondary prefix** (codec, non-blocking) — see the cell-codec
  TODOs.
- **One-transfer-per-tic enforcement** and **true row deletion vs. tombstone** — still
  open from `gaps.md` #3; pack/unpack inherits them.
