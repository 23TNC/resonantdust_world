# The object model — types, references, shards

The pinned contract for the 0.2.3+ redesign. This is the vocabulary and the
bit-level shape that everything downstream (shard tables, the tick pipeline, the
event log, the codec, the texture tree) builds against.

> **Status: DESIGN — in flux.** The reference bit-layouts and the shard-class
> model are settled enough to build against; the texture-taxonomy naming and four
> flagged decisions (§Open decisions) are still open. Supersedes the ad-hoc
> "object == thing" language throughout the codebase and reconciles
> [`object-shard.md`](../../../../archive/object-shard.md), [`data-shards.md`](../../../../archive/data-shards.md), and
> [`pipeline-generalization.md`](../../../server/spacetime/pipeline/design/pipeline-generalization.md) under one addressing
> scheme.

---

## 1. Vocabulary — stop overloading "object"

The word **object** was doing two jobs (a game thing *and* a general entity). We
split them:

- **object** — a *general* game entity. May be a tile, a thing, a pawn, a zone, a
  player, an event, a server. When we reason about tables and shards generically,
  we say "object." **An apple is not an object — it is a thing.**
- **type** — the class of object. `u4` field. Currently: `pawn`, `thing`, `tile`,
  `event`, `player`, `server` (+ `biome`, tentative — see §6). 16 slots, 0 reserved
  → 15 usable.
- **kind** — which specific variety an object's type is. e.g. a `thing` of kind
  `conifer`. `u10` field (see §3).
- **variant** — a purely-visual alternative of one kind (the art enumerates them).
  `u4` field.

Between type and kind, and between kind and variant, sit **subtype** and
**subkind** — see §3. They are real numeric fields, not just folder labels.

### "reference" = an indirect / packed identifier

Anything we used to call a *key* or *id* that is **not a direct value** — an
indirect table lookup, or several ids packed into one integer — is a
**reference**. A `*_id` is a single raw field; a `*_reference` is a packed
composite (or an indirection). This is the same "reference" vocabulary already
used by the event-log provenance schema (commit `81bb199`).

---

## 2. Spatial hierarchy

Four nested 16×16 grids. Every coordinate pair packs into a `u8` as `u4 · u4`.

| reference | bits | grid | spans |
|---|---|---|---|
| `position_reference` | `u8` = `u4 x` · `u4 y` | tile within a **zone** | 16×16 tiles |
| `zone_reference` | `u8` = `u4 zone_x` · `u4 zone_y` | zone within a **region** | 16×16 zones |
| `region_reference` | `u8` = `u4 region_x` · `u4 region_y` | region within a **realm** | 16×16 regions |
| `realm_reference` | `u8` = `u4 realm_x` · `u4 realm_y` | realm within the **world** | 16×16 realms |

Derived scales:

- **zone** = 16×16 = 256 tiles.
- **region** = 16×16 zones = **256×256 tiles ≈ one RimWorld map**.
- **realm** = 16×16 regions = **4096×4096 tiles**.
- **world** = 16×16 realms = **65536×65536 tiles** (a hard ceiling — the world is
  bounded, not infinite).

**`region_zone_reference` = `u16` = `region_reference` · `zone_reference`.** Stored
in cold rows so a subscription can specify region+zone directly, **without a
filter**. Realm comes from the shard (§5), so `region_zone` fully locates a zone
within its realm.

---

## 3. Object identity & instance — the packed object

An object is fully described by a single **`object_reference` = `u64`**, split into
two `u32` halves:

```
object_reference : u64
  ├─ object_type_reference : u32
  │     type_id     u4      (16, 0 reserved → 15 types)
  │     subtype_id  u12     (4096 subtypes per type)
  │     layer       u4      (16 object-SLOTS on a tile — see note)
  │     reserved    u12     (headroom)
  └─ object_kind_reference : u32
        kind_id     u10     (1024 kinds per subtype)
        subkind_id  u4      (16 subkinds per kind)
        variant_id  u4      (16 visual variants per kind)
        x           u4      ┐ position within the zone
        y           u4      ┘ (= position_reference)
        data        u6      (type-decoded payload — see note)
```

> **`data` is decoded per `type_id`** — up to 16 decode methods, extensible.
> *Default* (most types): `u2 rotation` (4 facings; W is mirrored E) + `u4 count`
> (magnitude 1–15). *Stackable / resource types:* the whole `u6` is `count`
> (magnitude 1–63) and the object doesn't rotate. Widen the vocabulary later
> without touching the word — the decode is a function of the type.

> **`layer` here is an object-slot, NOT the texture part.** This `layer` is *which
> object occupies a depth-slot on a tile* — one object per `(type, layer)` per tile
> (§4). It is unrelated to the texture path's `<part>` segment (§8), which is a
> *sprite piece of a single object* (pawn body vs head, composited together at draw
> time). One pawn has **one** object-`layer` but **several** texture-`part`s.

Design space: 15 types × 4096 subtypes × 1024 kinds × 16 subkinds × 16 variants.
Generous everywhere; the `u12` reserved in the type half is deliberate growth room.

**Why the split into two `u32`s.** The type half is *shared* across many objects
of the same (type, subtype, layer); the kind half is *per-instance* (its own
kind/subkind/variant + where it sits + its type-decoded `data`). Cold storage
exploits this directly (§4).

**Bit-packing trade to remember.** Because identity is a *position in the
type→subtype→kind→subkind tree*, **re-parenting the taxonomy re-mints ids**. Assign
ids append-only per level and never re-parent, or accept that reorganizing the tree
is a data migration.

---

## 4. Cold objects — geographic, packed, static

Cold objects are baked into terrain: scattered flora, rocks, settled resources.
They do not move on their own; they are addressed **by location**.

**Cold table shape** (lives in the **tile shard**, §5):

- key: `region_zone_reference : u16` + `object_type_reference : u32`
- value: `Vec<object_kind_reference : u32>` — every cold object of that
  (type, subtype, layer) in that zone, each carrying its own kind/position/etc.

Plus the dense terrain layer: **`Vec<u8>` of 256 tile entries per zone** (one per
tile — matches the existing `cold_tiles: Vec<u8>`). Tiles get a compact dense
encoding; things get the sparse packed-`u32` list.

**Subscription payload per zone:** 256 `u8` (tiles) + one `u32` per cold thing.

**Addressing a cold object — `cold_reference : u32`:**

```
cold_reference : u32
  region_reference    u8
  zone_reference      u8
  position_reference  u8
  layer_id            u4
  type_id             u4
```

This omits subtype/kind on purpose, which forces the **uniqueness rule**:

> **At most one object of a given `type_id` at a given `layer_id` on a given
> tile.** (Regardless of subtype.)

That rule is what lets `cold_reference` + one extra `u4 type_id` fully specify a
target. `cold_reference` alone doesn't say *which* shard in the realm holds the
zone, so a cold target is **`cold_server_id : u8` + `cold_reference : u32`**.

**Enforcement.** The rule is subtype-agnostic but `subtype_id` rides in the
*shared* type-half, so a `(region_zone, type, layer)` group is physically spread
across **one row per subtype**. A write therefore does a **reject-if-present**
check across those sibling subtype-rows: gather by `region_zone → type → layer → x
→ y`, reject the write if occupied (filtering by type first drops whole categories
cheaply — are we even looking at a thing vs a pawn vs a tile). Uniqueness is thus a
**reducer-enforced invariant, not a primary-key constraint** — the price of a
subtype-agnostic `u32` `cold_reference`, and cheap because it's indexed by
`region_zone + type_id`. (Columnar-with-unique-index is the alternative — structural
uniqueness + DB `WHERE` filtering — but it trades the compact `Vec<u32>` transport
for a row per object. Keep the Vec: a zone is 256 tiles.)

**Subscriptions never filter; queries do.** You subscribe to a `region_zone` (no
`WHERE`). *Within* that subscription you resolve a target by decoding the packed
fields and matching `type → layer → x → y` over the zone's small Vec.

**Stacking capacity.** For a **dedicated stackable/resource `type`**, `data`
decodes as `u6 count` = up to **63 per stack**, and one stack sits per `(type,
layer)` per tile with ~15 usable layers → ~**945** per tile. Plenty. (A resource
that instead shared a general `type` — e.g. `thing` — would compete for that type's
16 layers against trees, benches, etc., and get the default `u4 count` = 15; the
dedicated type is why stackables get the big `u6 count` decode.) Either way a large
pile is N layer-objects, not one stack of N.

### Representation: identity vs storage vs wire

Three separable things, so don't over-optimize the wrong one:

- **Identity** — `object_reference` (packed) is the **invariant**: it's the currency
  the action-DSL (u64 immediates), the texture resolver (path from type/kind/variant
  bits), and the registry all speak. Never un-pack it.
- **Storage layout** — an implementation choice. The compact `Vec<u32>`-per-zone
  (dense rows, cheap in ST) is the default; a columnar table (better `WHERE` +
  structural uniqueness) is a valid alternative. Pure byte-packing of each entry over
  ST's native encoding is a *marginal* win — the real ST win is few rows, not few
  bytes.
- **Wire** — the client hop is the one we optimize for bandwidth. **The edge packs
  the client wire**, decoupled from how shards store the data.

**The dense tile projection (parked optimization).** Tiles are `u32` objects like
everything else (uniform machinery, effectively unlimited authoring surface vs. the
old `u8` = 256-kind cap). But the tile layer is a **full dense 256-cell grid**, so on
the *wire* the position is the **array index** — `x/y` never need to travel, and
`rotation` drops for tile kinds that don't turn. That projects a tile from `u32` down
to ~`u16` (kind/subkind/variant/data), ~512 B/zone — nearly as tight as the old dense
`u8` array, without losing the authoring surface. A projection, not a separate type;
the stored object keeps its full identity. *(To investigate during codec/wire work.)*

---

## 5. Shards — realms, tiles vs objects, servers

**A shard belongs to exactly one realm.** `realm_reference` is a property of the
database, not stored per-row.

**`server_reference : u16` = `realm_reference : u8` + `server_id : u8`.** Within a
realm, `server_id` alone (a `u8`) distinguishes shards.

Two **shard classes** per realm — this is the cold/hot split made physical, and
the resolution of the original "one table, two conflicting models" problem:

| class | sharded by | holds | class of object |
|---|---|---|---|
| **tile shard** | **geography** (region_zone) | dense tiles + packed cold-thing rows | **cold** (static) |
| **object shard** | **shape** ("objects who share shape") | live entity rows keyed by a per-server handle | **hot** (mobile) |

The object shard being sharded **by shape, not geography** is load-bearing: a pawn
stays in its realm's pawn-shard as it walks around, so **within-realm movement
never changes servers**. Only crossing a *realm* boundary changes servers (§Open
#4).

**Per realm we spin up at least:** one edge, one worker, one tile shard, one
object shard. The **gate** attaches a player to an edge by the realm the player is
in; **edges** subscribe to `server_reference`s filtered by their `realm_reference`.

---

## 6. Hot objects — mobile, live handles

When a cold object is **minted** into a live one (release / unpack), it becomes a
hot object in an object shard.

- **`hot_object_reference : u32`** (a.k.a. `hot_reference`) — a per-server unique
  entity handle. Unique **only within its server**; pair with `server_reference`
  for global uniqueness.
- A hot target is **`hot_server_id : u8` + `hot_reference : u32`**.
- A hot object still *has* an `object_reference` (its full u64 identity), but its
  **live position is mutable component state keyed by `hot_reference`, NOT the
  packed `x/y/data`** in the word. The packed placement is only the *settle
  snapshot* — it goes stale the instant the object moves.

Because object shards are shape-sharded per realm, **objects do not move between
servers** except on a realm crossing. So a `hot_reference` is stable for the life
of the object within its realm.

---

## 7. Events — the generalized log

Events are objects too (`type = event`), which is why `event`/`server`/`player`
are types even though they aren't spatial things — it makes the event log just
more objects.

```
event
  event_reference   u32        (auto-populated; a u32 so it can be passed as an object_reference)
  tic               u32
  action            u16        (65536 action codes: pack / unpack / damage / move / …)
  worker_reference  u16        (the full server_reference of the worker assigned the job)
  server_reference  Vec<u16>   (servers the event touches; a u16 zero-extends to u32 if passed as an object_reference)
  object_reference  Vec<u32>   (the targets — cold_reference or hot_reference)
```

We carry the **entire** `u16` worker_reference and server_references (not just the
`u8 server_id`) out of caution, to leave room for inter-realm communication later.
`event_reference` is held at `u32` (rather than a plain `event_id`) so it can be
passed anywhere an `object_reference` is expected, and in case we later need to
pack something into it.

**The `action` defines the schema of every reference — there is no universal
actor.** A `cold_reference` and a `hot_reference` are both `u32`; the `action`
alone says what each slot of `object_reference` (and `server_reference`) means, by
**position**. The slots differ completely per action:

| action | object_reference slots |
|---|---|
| `move` | `[0]` pawn (hot), `[1]` tile |
| `mint_hot` (unpack) | `[0]` crate (cold) — no actor |
| `inspect` | `[0]` pawn (hot), `[1]` crate |

There is deliberately **no dedicated actor field**: "actor" isn't stable across
actions (`mint_hot` has none), so a positional, action-defined schema is the right
shape. Same for cold-vs-hot — no discriminator bit; the action's slot definition
says whether a slot is a `cold_reference` or a `hot_reference`. To touch a cold
object you must first make it hot, so `pack`/`mint_hot` take cold references and
most other actions take hot ones.

### Plans, and the edge as orchestrator

A player **intent** ("open the crate") is a **plan** — an ordered set of atomic
actions handled by workers. Responsibility splits:

- **Client** sequences the coarse steps: issue `move`; on completion issue
  `inspect`.
- **Edge** does **state-dependent expansion**: `inspect` targets a *cold* crate, so
  edge queues `mint_hot` (unpack) *before* the inspect. Edge is a per-realm
  planner, not just a router — it injects prerequisites from world state.

**Reference rebinding across an injected step.** The client's `inspect` names the
crate by its `cold_reference` (all it can know — it's cold). The injected
`mint_hot` **consumes that `cold_reference` and produces a new `hot_reference`**, so
the queued `inspect` must be **rebound** `cold_ref → hot_ref` before it runs. Pin
which: edge substitutes it in the queue, or the action references the crate
*symbolically* (by cold_reference) and resolves to the hot handle at execution.

> **Forward (worker/action execution model — next topic):** the client here
> round-trips *between* `move` and `inspect`. The alternative — client submits the
> whole plan `[move, inspect]` and edge runs + expands it — saves round-trips at the
> cost of edge owning more of the plan. Deferred to the execution-model pass.

---

## Decided

- **Terminology:** object / type / kind / variant; `*_id` (raw) vs `*_reference`
  (packed/indirect). "Object" is general; "thing" is a specific type.
- **Identity widths:** `type_id u4`, `subtype_id u12`, `kind_id u10`,
  `subkind_id u4`, `variant_id u4`. 0 reserved for `type_id` (null sentinel).
- **`object_reference u64` = `object_type_reference u32` + `object_kind_reference
  u32`**, with position (x/y) + a **type-decoded `u6 data`** packed into the kind
  half. Default decode = `u2 rotation` + `u4 count`; stackable types = `u6 count`.
- **Spatial:** four nested 16×16 grids; each pair is a `u8`; `region_zone` is a
  `u16` stored in cold rows for filter-free subscription.
- **Shard classes:** geographic **tile shard** (cold) vs shape **object shard**
  (hot), one realm per shard, `server_reference = realm · server_id`.
- **Cold uniqueness rule:** one object per (type, `layer`, tile), subtype-agnostic;
  **reducer-enforced** by reject-if-present, not a primary key (§4). Keep the
  `Vec<u32>` cold table (don't go columnar — a zone is 256 tiles).
- **Type-decoded `u6 data`** in the kind half (replaces fixed rotation+count): the
  `type_id` picks the decode (16 methods, extensible). Default = `u2 rotation` +
  `u4 count`; a dedicated stackable type = `u6 count` (63/stack, no rotation) →
  ~945 per tile across its layers. **No word widening.**
- **`layer` is two axes → renamed.** Object-model keeps **`layer`** (the tile
  object-slot: wire/DB/event-level). The texture sprite-piece reverts to **`part`**
  (authoring-only, §8). They are never the same field.
- **Events:** no discriminator bit and **no actor field** — `action` defines the
  positional meaning of every `object_reference`/`server_reference` slot (per-slot
  cold/hot, actor-or-not). To touch cold you `pack`/`mint_hot` it hot first.
- **Plans:** a player intent is an ordered set of actions; client sequences coarse
  steps, edge injects state-dependent prerequisites (cold target → `mint_hot`
  first). Edge is a per-realm orchestrator.
- **Hot handle:** per-server `hot_reference u32`, stable within a realm; live
  position kept outside the packed word.
- **Events** are `type = event`; the log is generalized objects.

## Open decisions

1. **Reference rebinding across an injected `mint_hot`.** A queued action naming a
   crate by `cold_reference` must be rebound to the `hot_reference` that `mint_hot`
   produces. Edge substitutes in the queue, vs. actions carry the target
   symbolically and resolve at execution. *(Small; part of the execution-model pass.)*
2. **Whole-plan vs step-by-step submission** (client round-trips between actions vs
   edge runs+expands a submitted plan). *Deferred to the worker/action execution
   model.*
3. **Realm crossing = server change = `hot_reference` re-mint.** *Deferred — handle
   cross-realm later.* Rare (every 4096 tiles) but a real seam: in-flight events
   targeting the object dangle. Needs a defined hand-off.
### Resolved while wiring worldgen

- **`variant` stays `u4` (16).** `flora` has 30 art variants but only 16 are
  reachable — accepted; the client renders modulo the kind's real count, and art
  caps at 16 per kind. Widening would steal bits from the full `u32` kind half and
  break other fields — **do not touch it.**
- **Tiles are cold rows, not a special dense array.** A zone is **N `ColdRow`s split
  by (type, subtype, layer)** — `biome-tile` rows (dense, one per cell) *and*
  `biome-thing` rows (sparse), all in the same shape. The tile layer carries its
  biome because that's the row's `subtype`. No parallel biome array, no widened tile
  cell. (Implemented: `Worldgen::zone_cold_objects`.)

---

## 8. Texture tree — the presentation side of the same taxonomy

The texture path mirrors the identity axes, so art and data share one vocabulary.

**New layout** (supersedes [`texture-paths.md`](../../../dev/textures/design/texture-paths.md)'s
`category.subcategory/kind.subkind` scheme):

```
textures/<type>/<subtype>/<kind>/<subkind>/<variant>/<dir>.<part>.<map>.<ext>
```

- Folders are **labels for the numeric axes** — `type/subtype` = the
  `object_type_reference` names, `kind/subkind` = the `object_kind_reference`
  names. Subtypes and subkinds are **required** (no implicit `.0`) for code
  simplicity.
- The leaf groups **by variant**: one `<variant>/` folder holds every direction ×
  part × map for that variant, e.g. `…/0/s.0.albedo.png`. This drops the old
  source-file `<id>` entirely and re-introduces a `dir.part.map` dotted filename
  (reversing the just-landed "map = exact filename" simplification — a deliberate
  trade so the resolver, which knows variant+facing, reads one folder).
- **`<part>` = a sprite piece of a single object** (e.g. `pawns/human/male` part
  0 = body, part 1 = head), all composited when that one object is drawn. It is
  **required**, and it is a *different axis* from the object-model `layer` (the
  tile object-slot, §3) — this reverts the 2026-07 `part`→`layer` rename precisely
  to end that collision.

**Renames implied by the new tree (LOCKED — subtype carries the biome):**

- `world/conifer` → `biome-thing/<biome>/conifer/default/<variant>` — **type =
  `biome-thing`**, **subtype = the biome** (`default`/`forest`/`tundra`/…), **kind =
  `conifer`**, **subkind = `default`**. Tiles use **`biome-tile`** the same way
  (`biome-tile/<biome>/grass/default`). This is *functional, not aesthetic*: a cold
  row already carries `subtype_id`, so making subtype = biome means the row *is* the
  zone's biome — worldgen's biome selection maps straight to `subtype_id`, no
  parallel classifier, and `u12` subtype = 4096 biomes. It absorbs the worldgen
  `biome` (doesn't collide with it).
- `conifer`/`berry`/`flora`/`grass`/… are **kinds** (`kind_id` global per type — one
  id regardless of biome; the existing `tile_def_id`/`thing_object_id` *are* those
  kind ids).
- `pawns.human/male.fit` → `pawn/human/male/fit/<variant>` (pawn subtype = species,
  not a biome).
- `linked` (autotile walls/fences/rocks) is a **rendering** trait, not a type → a
  per-kind autotile flag under a real type (still open, below).

The `type`/`subtype` name↔id tables live in **`content/registry.rd`**
(append-only, 0-reserved); kind ids come from the tile/thing buckets. See §Decided.

**Sprite sheets** (multi-blob sources that aren't per-variant) stay loose at the
level they span, named by the axes the path doesn't already fix; the `<atlas>`
still populates subkind/variant as today.

### Open (art/dsl), still owed

- **Registry** — ✅ `content/registry.rd` (type + subtype tables, append-only,
  0-reserved). Kind ids from the tile/thing buckets, global per type. Loader wiring
  to compose `object_type_reference`/`object_kind_reference` from it: next.
- **`linked`'s home** — autotile flag under a real type vs texture-only pseudo-type.
  Still open (not on the biome-content critical path — `rock` is a `biome-thing`
  kind; walls/fences are future built structures).
- **Variant 0** — keep `variant` 0-based (reserve 0 only for `type_id`); existing
  art is 0-based on disk.
- **The sheet-placement rule** re-mapped onto the 4-level tree (draft into
  `texture-paths.md`).
- **DSL stem contract** changes: `content/visual/things.rd` stems like
  `"world/conifer"` become `biome-thing/<biome>/conifer/default`; server
  (`tex_manifest.rs`, `textures.rs::master_albedo_rel`), client
  (`pixijs/src/textures/*`), `texpath.py`, the migration script, and R2 keys follow.
