# Variables

> **AUTHORITATIVE** for every cross-component variable's name, width, and bit layout. Anything that
> disagrees — a doc, a comment, the code — is the bug.
> Rationale, history, and what was removed: [`notes/variables.md`](notes/variables.md).

Indentation = decomposition. Children are listed **high bits first**. A parent's width is the sum of
its children.

---

## Primitives

```
u8 <spatial>_reference            realm | region | zone | tile all use this shape
  u4 x                            bits 4–7
  u4 y                            bits 0–3
```

`ZONE_DIM` = `REGION_DIM` = `REALM_DIM` = 16. `ZONE_TILES` = 256.

---

## What it is

```
u32 definition_reference
  u16 type_reference              bits 16–31
    u4  type_id                   bits 28–31    0 = none
    u12 subtype_id                bits 16–27
  u16 kind_reference              bits 0–15
    u12 kind_id                   bits 4–15
    u4  variant_id                bits 0–3
```

16 types × 4096 subtypes × 4096 kinds × 16 variants. **No `subkind`** — these four fields are the
whole definition; the texture folder taxonomy matches 1:1 (`type/subtype/kind/variant`, see
[texture-layout](components/dev/textures/design/texture-layout/README.md)).

### `kind_id` partition — ground tiles vs linked objects (biome-tile)

`TYPE_BIOME_TILE` carries **both** ground tiles **and** linked/constructed objects (walls, fences,
rocks, blueprints) so they ride the **one dense tile vector** per zone (the point: a brick wall and a
grass floor are the same row shape in the same dense vector). `kind_id` (u12) is split by its top bit:

| `kind_id` | is | `kind` names | `variant` = |
|---|---|---|---|
| `0x000–0x7FF` (2048) | ground tile | material (grass, dirt, sand, water…) | art variation `0..15` |
| `0x800–0xFFF` (2048) | linked object | material / blueprint (smooth, brick, plank, metal, flecked; blueprint) | form (wall, fence, rock…) |

For **every** biome-tile object: **`subtype_id`** = the biome (`0x000` = default, 4096 biomes);
**`kind_id`** = the material (or `blueprint`), tile-half or linked-half per the split; **`variant_id`**
= the form for linked, or the art variation for a plain tile. Only 16 variants fit the `u4` — extra
on-disk variations may exist but **truncate out of the manifest** (`variant_id ≥ 16` unused). A
scattered `TYPE_BIOME_THING` (e.g. `conifer`) uses the same shape: `subtype` = biome, `kind` = the
thing, `variant` = art variation. `TYPE_THING` (7) stays for genuinely biome-invariant freestanding
objects; walls/fences/rocks do **not** use it — they are biome-tile.

---

**16 variants per `(type, subType, kind)`, and two different overflow behaviours — do not conflate
them** (definition-registry [F13](work/2026-08-04-definition-registry/forks.md#f13)):

- **Extra ART on disk truncates**, silently and by design (above). The art tree may hold more
  variant folders than the id can address; the index simply stops at 16.
- **Extra AUTHORED variants are a load error.** A corpus block whose `variant` array reaches a 17th
  slot is refused by the allocator rather than wrapped — wrapping would alias two definitions onto
  one id, which is the one thing the registry exists to prevent.

`pawn/animal/wolf` currently holds **15** art variants, so it is one slot from the ceiling. A kind
that outgrows it splits into more kinds; the packed layout does not move.

## Where it is

```
u32 position_reference
  u16 macro_position_reference    bits 16–31
    u8 region_reference           bits 24–31
    u8 zone_reference             bits 16–23
  u16 micro_position_reference    bits 0–15
    u8 tile_reference             bits 8–15
    u8 layer_reference            bits 0–7
      u4 type_id                  bits 4–7
      u4 layer_id                 bits 0–3
```

No realm. One object per `(type, layer, tile)` within a zone, subtype-agnostic.

**World structure.** Every geographic level is a `u4` nibble pair (`x:4 | y:4`), so every edge is 16:

| const | value | meaning |
|---|---|---|
| `ZONE_DIM` | 16 | tiles per zone edge (`tile_reference = tile_x:4 \| tile_y:4`) |
| `REGION_DIM` | 16 | zones per region edge (`zone_reference`) |
| `REALM_DIM` | 16 | regions per realm edge (`region_reference`) |
| `ZONE_TILES` | 256 | cells per zone (the dense tile array's length) |
| `REGION_TILES` | 256 | tiles per region edge (`REGION_DIM · ZONE_DIM`) |
| `REALM_TILES` | 4096 | tiles per realm edge |

A realm is 4096 tiles/axis and **only realm 0 is used**. Grow the world by widening a reference
(`u8 → u16`: a level `4 → 8` bits, `DIM 16 → 256`) or lighting up realms — the math reads these
constants, so it doesn't change. A zone's **world-tile origin** is `macro_world_origin(macro)` =
`(region_x·REGION_TILES + zone_x·ZONE_DIM, region_y·… + zone_y·…)`, straight from the region/zone
nibbles. Constants + helper are authoritative in `shared/codec/object`; legacy `packed` re-exports them.

---

## Which one it is

```
u8 server_reference
  u4 type_id                      bits 4–7      the object type this server serves
  u4 server_id                    bits 0–3

u24 object_reference              an opaque per-server minted id

u32 entity_reference
  u8  server_reference            bits 24–31
  u24 object_reference            bits 0–23

u16 realm_server_reference        cross-realm only
  u8 realm_reference              bits 8–15
  u8 server_reference             bits 0–7

u16 route_reference               cold-routing key: which family, which region
  u4 type_id                      bits 8–11     TYPE_BIOME_TILE / TYPE_BIOME_THING / …
  u8 region_reference             bits 0–7      (bits 12–15 reserved)
```

`SERVER_REF_NONE` = 0. `OBJECT_REF_MAX` = 0xFF_FFFF.

An `entity_reference` is realm-unique only via its `server_reference`; crossing realms, carry a
`realm_server_reference`.

**Aliases of `entity_reference`** (u32, same layout, no interior):

| alias | names | `type_id` | on |
|---|---|---|---|
| `event_reference` | an event row | `TYPE_EVENT` | `event_log.event_reference` |

**Aliases of `server_reference`** (u8, same layout, no interior):

| alias | names |
|---|---|
| `worker_reference` | the worker that writes a piece of work |
| `observer_reference` | the worker that reads a `state_log` row as its next tic's base |
| `orchestrator_reference` | the orchestrator that groups a tic's events |
| `shard_reference` | a shard |

**Aliases of `realm_server_reference`** (u16):

| alias | names | on |
|---|---|---|
| `player_shard_reference` | the shard serving a player's data | `players.player_shard_reference` |

---

## Time

```
u16 tic                           the simulation clock; WRAPS

u64 state_uid                     the (entity, tic) slot — PK of state_log
  u16 reserved                    bits 48–63
  u32 entity_reference            bits 16–47
  u16 tic                         bits 0–15

u64 cold_uid                      the (cold_row, tic) slot — PK of BOTH dense_log and sparse_log
  u16 reserved                    bits 48–63
  u32 cold_row_reference          bits 16–47
  u16 tic                         bits 0–15

u64 event_uid                     the (zone, tic, event) row — PK of event
  u16 macro_position_reference    bits 48–63
  u16 event_tic                   bits 32–47
  u32 event_reference             bits 0–31
```

`entity_reference` above `tic`: **entity-major**. One entity's slots are contiguous; `(entity, tic)`
is an exact key. Tic is low because it wraps — no key ordering can give a sound tic range.

A ring, not a line. **Never compare with `<` / `<=`** — across the wrap they invert (`0` is after
`65535`). Use serial arithmetic: `(a.wrapping_sub(b) as i16)` — positive = `a` after `b`.
`shared/codec/src/tic.rs`.

`TIC_WINDOW` = 32767 — two tics further apart than this cannot be ordered.

---

## Cold storage — the `DenseItem` / `SparseItem` row items

A cold shard's baseline (`entity_state`/`entity_state_log`) and overlay (`overlay`/`overlay_log`) come
in **two forms** (both keyed by `cold_row_reference`, promoted by `PROMOTE`). The forms differ only in
the **item** their `items: Vec<_>` holds — generic over an optional user payload `<T>`:

```
u32 cold_row_reference            realm-unique within its module (the module's type_id IS the shard)
  u16 macro_position_reference    bits 16–31
  u12 subtype_id                  bits  4–15
  u4  layer_id                    bits  0–3

struct DenseItem<T>               dense_entity_tables! — one entry PER CELL, ZONE_DIM² (index = tile_reference)
  u16 kind_reference              ALWAYS present (fixed) — what object the cell holds
  T   payload                     the optional EXTRA; none for tile, `u8 data` for a dense-with-data

struct SparseItem<T>              sparse_entity_tables! / overlay_tables! — one entry per OCCUPIED cell
  u8  tile_reference
  u16 kind_reference              ALWAYS present (fixed)
  T   payload                     the optional extra; `u8 data` for thing

u8 data                           the usual thing payload: rotation:2 | count:6 (west mirrors east)
```

Row identity = `(macro_position_reference, subtype_id, layer_id)` = the `cold_row_reference`. **The
`type_id` is the shard** (`tile` *is* `TYPE_BIOME_TILE`, `thing` *is* `TYPE_BIOME_THING`) — out of the
key and off the row, which freed the `u12 subtype_id`. A zone with N biomes is **N rows**, one per
`subtype`. Reconstruction: `type_reference` = `shard.type_id:4 | row.subtype_id:12`;
`definition_reference` = `type_reference:16 | item.kind_reference:16`; `layer_reference` =
`shard.type_id:4 | row.layer_id:4`; a cell's `position_reference` = `row.macro:16 | item_or_index
tile_reference:8 | layer_reference:8`.

**Dense vs sparse addressing.** A **dense** row allocates the full `ZONE_DIM²` items, so a
`tile_reference` is a **direct index** into `items`. A **sparse**/overlay row holds only occupied
cells, so a `tile_reference` **selects** the item whose `tile_reference` matches (each `SparseItem`
carries its own). Actions resolve a cell this way — index for dense, match for sparse.

**Both forms are `*_log`/`*` pairs**, keyed by `cold_uid` (`cold_row_reference:32 | tic:16`, above) with
the shared composition columns (`worker_reference`, `observer_reference`, `status`). A shard's
**baseline** is `dense_entity_tables!` (tile) or `sparse_entity_tables!{data}` (thing) →
`entity_state`/`entity_state_log`; its **overlay** is a **second sparse table**, `overlay_tables!` →
`overlay`/`overlay_log`, because an override touches only a few cells. (The hot, `entity_reference`-
addressed `entity_tables!` is for hot shards only.) Nothing writes any of these directly — a worker
composes `*_log`, a [`PROMOTE`](ACTIONS.md) prefix projects it to `*` (smart + atomic), GC `PACK`s the
settled `overlay` into `entity_state_log`. Client renders **`entity_state` ⊕ `overlay`** — an overlay
cell shadows its baseline cell (no per-cell `tic` compare; atomic promote prevents any flash).

> **Naming.** The row id keeps the name `cold_row_reference` (and `cold_uid`) even though the tables are
> now `entity_state`/`overlay` — "cold" survives only in the reference name, a later cleanup.

---

## Status bytes

```
u8 event_status                   event_log.status, event.status
  u4 flags                        bits 4–7
  u4 status                       bits 0–3

u8 state_status                   state_log.status
  u4 flags                        bits 4–7
  u4 status                       bits 0–3
```

`event_status.status`: `QUEUED` 0 · `GROUPED` 1 · `ASSIGNED` 2 · `RUNNING` 3 · `COMPLETE` 4 — the
phase. `QUEUED` = created; `GROUPED` = the shard put it in a local `event_group`; `ASSIGNED` = the
orchestrator gave its work-group a worker; `RUNNING`/`COMPLETE` = the worker.
`event_status.flags`: bit 0 `FAILED` · bit 1 `PROMOTE`.

`state_status.status`: `OPEN` 0 · `PROMOTED` 1.
`state_status.flags`: bit 0 `PROMOTE`.

Composition-settled is `state_log.dirty == false` (a boolean, one worker per component); `PROMOTED`
is the separate fact that the value reached `state`.

---

## Enumerations

**`type_id : u4`** — append-only.

| name | value |
|---|---|
| `TYPE_NONE` | 0 |
| `TYPE_BIOME_TILE` | 1 |
| `TYPE_BIOME_THING` | 2 |
| `TYPE_PAWN` | 3 |
| `TYPE_PLAYER` | 4 |
| `TYPE_EVENT` | 5 |
| `TYPE_SERVER` | 6 |
| `TYPE_THING` | 7 |

---

## Textile slot grid (`client/webgl`)

**Every textile map is sized in TILES, never in screen resolution, and never changes size.** Work stream:
[`work/2026-07-26-textile-slot`](work/2026-07-26-textile-slot/README.md).

A fixed grid of **32 × 16 SLOTS** (28×12 visible + **2 slots of overscan per side**). A slot holds **1 tile
at PARTITION LEVEL 0** and **`2^k × 2^k` tiles at level k**, so the texture is constant while the world it
covers grows 4× per step. (`2026-08-02-one-resolution` P4: this concept was formerly ALSO called "lod";
the atlas LOD ladder is deleted and the **partition level** is the renamed survivor. `SQUARE = 128` caps
what a level-0 slot DISPLAYS; art now packs at its manifest max regardless.)

```
SLOTS_X = 32   SLOTS_Y = 16      VISIBLE_X = 28   VISIBLE_Y = 12   OVERSCAN = 2
PARTITION_LEVELS = 3             (level 0..2 — fits u2, with room to restore level 3)
REFERENCE = 3584 × 1536          (the visible slots at level 0)
```

**`SLOTS` is a POWER OF TWO on both axes, deliberately.** The toroidal wrap is `mod(wc, SLOTS << level)`,
which stays pow2 at every level, so it compiles to a bitmask rather than an integer division. Correctness
does not depend on it — a slot subdivides into `2^level` tiles at any grid size — purely a cost property.

**MODULUS vs STRIDE.** The `textile_square` TEXTURE is `SLOTS + 2` slots per axis (34×18 = 4352×2304). The
extra ring is the **wrap-apron**, applied as a `(sx + 1)` offset AFTER the modulus, so it never enters the
wrap arithmetic: the toroidal window straddles the texture edge, and `bakeSquare` mirrors an edge slot to
the opposite border so a square adjacent across the wrap has physically adjacent texels. A non-pow2 texture
costs nothing (WebGL2 handles NPOT at NEAREST/CLAMP); the address math stays pow2 regardless. The
`textile_unit` and `textile_tile` maps carry no apron.

**One grid serves every map; only texels-per-slot differs** — which is why the partition level can be
published once in the constants px and read by every shader, instead of each deriving a slot address from
a world coord (the recurring failure
[`work/2026-07-24-map-compatibility`](work/2026-07-24-map-compatibility/README.md) exists to police).

| family | texels/slot | texture | maps |
|---|---|---|---|
| `TEXTILE_SQUARE` (per px at level 0) | `SQUARE` = 128 | 4352 × 2304 (incl. apron) | albedo, normal, surface, zdepth — **× 2, cold AND warm** |
| `TEXTILE_LIGHT`, no apron | **64, PINNED** | 2048 × 1024 | lightmap (cold + hot), fine receiver |
| `TEXTILE_UNIT` (per unit) | 16 | 512 × 256 | shadow (cold/hot + both prev), coarse receiver, decay |
| `TEXTILE_TILE` (per tile) | 1 | 32 × 16 | presence, caster buckets, dirty |

**`TEXTILE_LIGHT` is PINNED and does NOT track `SQUARE`** (work
[`2026-07-28-square-128`](work/2026-07-28-square-128/README.md)). `SQUARE` is the **art** dial and only
the art dial. The two were one constant until 2026-07-28, which meant a cheaper lighting pass could only
be bought by paying with art resolution — the trade the 2026-07-27 A/B was forced into. Measured, holding
`SQUARE` fixed and moving only the lighting resolution 64 → 128: the lighting draw costs **2.2–3.1×**
and the display cannot show the difference (the blit samples it NEAREST; the detail the eye reads is in
the albedo). Three maps ride `TEXTILE_LIGHT`, so the pin is worth **216 MiB** at `SQUARE = 128`.

Measured resident bytes at `SQUARE = 128`: art **306 MiB** (8 × 38.25), lighting **72**, shadow **11**,
atlas **16** — **405 MiB** total, against **621 MiB** if the two dials were still joined.

**Fit is COVER, not contain:** `s = max(W / 3584, H / 1536)`. `max` (not `min`) is what keeps the viewport
entirely inside the visible slots; `min` would fit the whole grid and expose overscan at the edges.

| level | tiles/slot | tile texels | visible tiles | zoom band (s=1) |
|---|---|---|---|---|
| 0 | 1×1 | 128 | 28×12 | [1, 2) |
| 1 | 2×2 | 64 | 56×24 | [0.5, 1) |
| 2 | 4×4 | 32 | 112×48 | [0.25, 0.5) |

`ZOOM_MIN = 0.25` stops the ladder at level 2. Level 3 (8×8 tiles/slot, 224×96 visible) is expressible —
the `u2` field has room and no layout changes — but its window is ~450 zones, which needs a
**loading-priority system** before it is pleasant. Deliberately deferred, not designed out.

At a 16:9 display the VERTICAL axis binds (`1440/1536 > 2560/3584`), so a 2560×1440 player sees 120-px tiles
and 21.3×12 of the 28×12 visible slots — the horizontal slack is cached-but-offscreen, which is the correct
axis to spend it on since the vertical budget is the scarcer one.

**The partition level does not affect display sharpness.** Texels per screen pixel is
`(128/2^k) / (σ · 128/2^k) = 1/σ` — the `2^k` cancels. The level selects *world coverage* only; sharpness
is governed by the viewport against the 2560×1536 reference. The **bake** stage samples the one
max-size master through the GRAPHICS page's capped mip chain (one-resolution P3/F4 — the ladder of
pre-shrunk files is gone; the DATA page stays NEAREST/exact).

**Rescaling on a level change is NEAREST** — replication up, decimation down, never averaging. Independently
mandatory for three maps, which is what permits one shared reproject path: the shadow bitfield cannot be
filtered at all, `zdepth` encodes a discrete `0x80 | baseRow` that averaging silently corrupts, and the
lightmap must stay exactly-representable to remain invertible. Linear is permitted for albedo/normal/surface
only.

## Lighting data textures (`client/webgl`)

_Shipped by [`work/2026-07-31-lighting-rework`](work/2026-07-31-lighting-rework/README.md), verified @ 5e33c84a._

**Everything is a prim.** One flat `u16` index space — no `set` nibble, no billboard/light/tile
taxonomy. Three type lanes say what a prim *does*: `cast_type`, `receive_type`, `emit_type`. **Index 0
is the global sentinel** in every index space, so one comparison covers "empty slot", "no caster",
"unresolved definition", and no magic value is carved out of the `u16` range.

```
prim_data                                          1 px per prim, RGBA32UI
  R  u16 unit.x            | u16 unit.y
  G  u8  unit.z (24-31)    | u4 fine.x (20-23) | u4 fine.y (16-19)
     u4  fine.z (12-15)    | u8 seed (4-11)    | u4 rotation (0-3)
  B  u2  cast_type (30-31) | u2 receive_type (28-29) | u2 emit_type (26-27)
     u4  reach (22-25, BIASED +1 = 1..16 tiles) | u6 intensity (16-21)
     u16 definition_index (0-15)
  A  u8  color.1 | u8 color.2 | u8 color.3 | u8 color.4      (.4 = emitted light colour)

definition_data                     16 sequential px per definition, indexed by ROTATION
  R  u12 frame.x | u12 frame.y | u4 frame.span | u2 anchor.x | u2 anchor.y
  G  (free — held at 0; reserved for the ANCHOR lane, see below)
  B  u2  cast_type (30-31) | u2 receive_type (28-29) | u2 emit_type (26-27)
     u4  layer (22-25)     | u8 seed (14-21) | u4 rotation (10-13)
     u4  reach (6-9, BIASED +1) | u6 intensity (0-5)
  A  u8  color.1 | u8 color.2 | u8 color.3 | u8 color.4
```

`frame.span` is **biased**: 0 means 1, 15 means 16. A span of 0 would be no frame, so the natural
encoding wastes the one value it cannot use and gets the maximum wrong by one.

**GREEN held a SUBFRAME and is now free** (2026-08-02). It carried the art's opaque bbox in whole
units, and every card was placed and silhouetted through it, bottom-aligned to the prim's anchor —
while that anchor was the *continuous* opaque bottom. Two derivations of one edge, one rounded and
one not, drifting up to a unit and painting a halo around every sprite
([`work/2026-08-02-normal-frames`](work/2026-08-02-normal-frames/issues.md#i8)). Placement and
sampling now both address the **frame**, which carries no rounding and is the rect the bake draws.

The crop moved to atlas **ingest**, from a corpus-authored subframe
([`work/2026-08-02-subframe-ingest`](work/2026-08-02-subframe-ingest/README.md)) — see the
subframe variables below. The lane is reserved for the **anchor** (`u16 x | u16 y`, sixteenths of a unit),
which is what lets a prim's plan line sit on its art's feet rather than on its frame's bottom edge.

**The two BLUE channels are NO LONGER the same layout** (z-positioning P0b). `definition_index` moved
from GREEN to BLUE's clean low 16 bits, paid for by moving `seed` and `rotation` to GREEN and retiring
`layer`; a definition has no `definition_index` to hold and still needs `seed` where it is, because
`silhouetteHit` reads it as `pxPerUnit × 8`. So the two records diverge here on purpose.

**"A prim copies the definition's BLUE" now means FIELD-WISE, not word-wise** — which is what it
always was in practice: `RecordSync` passes `cast_type`/`receive_type` from the resolved definition
into `writePrim`, and each record packs from the field NAMES independently. Nothing ever copied the
raw word, which is why the split is safe.

**`unit.z` is a HEIGHT above the ground, in units, and the record holds GAME coordinates.** `unit.x`
and `unit.y` are where a prim physically **stands** — not where it is drawn. The drawn row is
`unit.y − elevation`, a 1:1 shift with no coefficient, so a pawn's head and its body carry the
**same `unit.x/y`** and differ only in `unit.z`. That is what makes their shadows align by
construction rather than by tuning (work `2026-08-01-z-positioning`, F7).

**`fine.z` is game-space, in units**, like `fine.x`/`fine.y`: the full elevation is
`unit.z + fine.z/16`. It refines the ELEVATION, not a projected quantity.

**One meaning, one conversion (F9).** The lane stores a **drawn** up-screen shift. A light's authored
height is a *world* height, so `RecordSync` converts it on the way in — every reader downstream then
compares like with like. The tilt enters in exactly two places: turning a **drawn extent** into a
world elevation (`× sin(world_tilt)`, the factor `shadowGather.ts` used and
`content/things.toml` still documents), and `screen.z = tan(world_tilt) · unit.z`, which is a
depth perpendicular to the screen used **only** for z-ordering, never as a height.
`client/webgl/src/game/viewport/worldTilt.ts` is the one place the angle appears.

`base + rotation` is the whole addressing rule for art. It subsumes the n/s perpendicular caster card,
the e/w mirror and the 16-cell autotile table: three special cases collapse into one add.

**Reach is STORED** (lighting-correctness P1, user directive), taking four of intensity's old ten
bits: `u4` biased +1 → **1..16 tiles**. Sixteen caps the lane deliberately — reach is the measured
cost dial (walk length ∝ reach, claimed texels ∝ reach², overlap multiplies both), and 16 is the
ceiling every headline number is priced at; the encoding refuses the value the system is known to
choke on. Reach bounds registration + walks; the falloff `L(d) = I / (1 + (d/d0)²)` (`d0` = 1 tile)
shapes light WITHIN it, so a short reach on a bright light clips visibly — that is content's dial,
exactly as the old authored `&thing.light.reach` behaved. The CPU tile-registration and the GPU walk
bound both read the SAME lane via `records.ts`'s `LIGHT_LANES_GLSL` (`reachUnitsFromB`), which is
what keeps them agreeing by construction (`lightReach.ts`'s derive-and-share is retired).

```
light        1 px per TILE, RGBA32UI   8 x u16 prim indices — the 8 nearest emitters reaching it
presence     1 px per TILE, RGBA32UI   8 x u16 — slot 0 = the tile; 1..7 receivers, LAYER-SORTED
shadow       3 px per UNIT, RGBA32UI   ping-ponged
  px 0   8 x u16 — the caster occluding each light, for the ground
  px 1   4 x (u16 caster, u16 receiver) — lights 0..3
  px 2   4 x (u16 caster, u16 receiver) — lights 4..7      (the split is `l >= 4`)
```

`presence` being layer-sorted is load-bearing, not cosmetic: the resolution order walks it topmost-
first and takes the first receiver covering the pixel. Over-cap eviction keeps the **topmost**.

**The ONE z contract** (lighting-correctness P4): the presence sort key IS the draw's `zIndex` — the
same number the painter orders sprites with — so the surface a pixel is LIT as is the surface it is
DRAWN as, by construction (`writePresence` rejects non-finite keys).

The prim `layer` LANE is **retired** (z-positioning P0b) — it carried the pawn part slot and nothing
ever read it back, so it was write-only data occupying the nibble `fine.z` now needs. World z-order is
untouched: it lives in the presence SORT, keyed by the same `zIndex` the painter draws with.
`definition_data` keeps its own `layer` lane.

```
light slots  LIGHT_SLOTS px per lighting texel, RGB10_A2   one light each, value stored /4
summed map   1 px per lighting texel, RGBA32F              integer QUANTISATION LEVELS
```

Slots are stored at **¼ scale** because the display clamps at 4.0 — 4× overbright is shipped
behaviour, and a 0..1 format would clip each light *before* the sum. The summed map holds **integer
levels, not floats**: a deposit and its later withdrawal then cancel bit-exactly (integers below 2²⁴
are exact in FP32), which is what makes removing a light an exact operation rather than an
approximate one. Slots need no such care — they are overwritten, never accumulated.

**Torus addressing** (lighting-visual P4, user directive): every lighting-chain map — the slot
map, the summed map, the receiver map and the shadow buffer — rides the SAME slot torus as the
composites: a tile's texel block sits at `mod(tile, cols/rows) × texelsPerTile`, the texture never
changes size, and texels-per-tile HALVE per partition level (`TEXTILE_LIGHT >> level` for the lighting maps,
`TEXTILE_UNIT >> level` for the shadow buffer). Writers unwrap a fragment's residue to its unique
window tile (`fillDisplay`'s rule); the blit reads the same residues, and its bilinear taps wrap
by `pmod`, which lands them on the WORLD-adjacent tile — the torus makes the seam free. Falloff is
`lightFalloff(d, reach, I)` in `LIGHT_LANES_GLSL` — d₀ = reach/2 with a linear feather to exactly
0 AT the stored reach, so the registration boundary and the visible pool edge are one line.

## Sprite subframes — the atlas crop (`shared/content` → `client/webgl`)

**One rect, authored once, cropped at ingest, shared by all four maps.** Work stream:
[`work/2026-08-02-subframe-ingest`](work/2026-08-02-subframe-ingest/README.md).

A texture master is a **square pow2 canvas with the subject letterboxed** inside it
(`bin/art`'s normalisation). The **subframe** says which fraction of that canvas is actually art.
The atlas ingests exactly that rect out of each of the stem's four maps — albedo, normal, surface,
layers — so they are registered with each other **by construction** rather than by two derivations
agreeing.

```
[thing.part.subframe]                       per PART SLOT (a head is its own master)
default = { x, y, w, h, ax?, ay? }          fractions 0..1; default = the whole frame
e / s / n / rN = { … }                      per-ROTATION override (aliases r0..r3)
vN = { … } / "vN.e" = { … }                 per-VARIANT / fully-specific overrides
sprite_anchor = { x, y }                    the flat pivot the chain's ax/ay tail falls back to
```
(TOML spellings — § TOML content schema; the retired `&thing.subframe.*` DSL forms are in git.)

Exposed as `thingSubframe()` — **stride 18** per def, `[e, s, n] × [x, y, w, h, anchor.x, anchor.y]`
— and on each `moverParts()` slot as `subframes` (the same 18 floats, so one decoder serves both).

**FRACTIONS, never units.** A fraction addresses the same art at ANY served size (a unit count
silently means a different number of texels per size — and one-resolution now packs each stem at
its manifest max). Whole units are also precisely what the deleted `definition_data` GREEN lane held, and rounding them
against a continuous anchor is what put a halo around every sprite.

**THREE directions, not four.** West is the east master *mirrored*, not a fourth texture, so the
client derives it by mirroring `e` about the frame centre. A fourth row would be a second number
describing one image — the failure this model exists to remove.

**Per direction because a facing is a different texture.** The corpus's clearest case is the wolf:
its side view is long and flat (`h 0.47`) and its front/back are tall and narrow (`w 0.33`). One
rect could not serve both.

**Authored from the MASTERS ON DISK**, never from the client: the runtime bbox was computed from
whichever size decoded first, so the same art measured differently between sessions. Masters are one
size (`meta.json` `"square": 128`) and measure the same every time. The union across a kind's
variants is taken, so no variant is clipped.

**`internal_padding` was deleted into this** — a uniform inset *is* a uniform subframe, so a linked
tile grid authors its inset as a subframe like everything else.

## Needs & conditions (`shared/content` → shard / npc / `client/webgl`)

**The pawn's hidden state and its displayed consequences.** Work streams:
[`work/2026-08-03-needs-moodlets`](work/2026-08-03-needs-moodlets/README.md) (the model),
[`work/2026-08-04-conditions`](work/2026-08-04-conditions/README.md) (the word — `moodlet` was
renamed to **condition** everywhere; no wire value moved) and
[`work/2026-08-06-interactions`](work/2026-08-06-interactions/README.md) (f32 values, registry
ids, the drink producer). A **need** is an **f32 SATISFACTION on its OWN authored domain**
(`min..max` — interactions F7; the corpus decides range and sign, the system stores and CLAMPS)
depleting toward `min` (needs-moodlets F1 — one dialect for every need; bad states are LOW; the
player never sees the scalar). A **condition** is what displays: a label plus its effects. Mood
is `clamp(0.5 + Σ active offsets, 0..1)` (F5).

**A condition's effects are an OPEN set** (conditions F6) — `mood` is the FIRST of them, not the
definition of the thing. Conditions act on pawn state, and the user's own framing is that they
reach further than mood: a need's depletion rate, later a stat. Add fields beside `mood`; do not
write code or docs that assume a condition *is* a mood offset. The effect table is the successor
stream's charter, alongside the still-pending drink action.

```
[[need]]       name/label · min/max (the authored value domain, f32) ·
               deplete (TICS max→min; 0 = never drains) ·
               band = [{ condition, lo, hi }]  in the need's OWN units — exclusive, ≤1 active
[[condition]]  name/label · mood (-1..1) · duration (TICS a TIMED grant lives; 0 = DERIVED) ·
               priority (sort key; see below)
[[thing]]      needs = ["thirst", …] · traits = ["biological_lifeform", …]
```
(TOML spellings — § TOML content schema. Ids are REGISTRY-allocated from the derived
`gameplay/<category>/<name>/default` taxonomy — interactions F1/F9; payload words and event
inputs carry the full u32 `definition_reference`.)

Exposed as `need_params_all()` / `condition_params_all()` (registry order) and
`thing_needs_table()` — **stride 8** per kind of u32 need `definition_reference`s, `0` = empty
slot.

**`priority` orders the display, and it is AUTHORED** (conditions F2). The details panel maximizes
the top 4 conditions and minimizes the rest, so the ranking is a game-design decision and lives in
the corpus where it can be tuned without a rebuild. Author in **tens** (10/20/30) so a new
condition slots between two without renumbering; absent = `0`.

**The sort is `priority` desc → `|mood|` desc → `condition_id` asc**, and it is computed **once**,
inside `needs_eval::active_conditions` (F3) — never in a consumer. Every observer therefore ranks
identically: the panel's card order and the npc's decision order are the same list. The two
tie-breaks make it a TOTAL order, so a pawn whose state has not changed cannot have its cards swap
places between evaluations. `pawnConditions` (wasm) returns **stride 4** —
`[condition_id, mood, remaining, priority]` — already in that order; `priority` rides along to be
*shown*, not to be re-sorted. Do not derive priority from `|mood|`: it cannot express "mild but
urgent", and it degenerates entirely once a condition's effect is a need rather than a mood.

**Nothing ticks a need** (F4): a pawn's shard row is `(satisfaction: f32, set_tic)`; observers
compute `satisfaction_at(tic)` and every band-crossing tic from `deplete` over the authored
`min..max` domain. The two kinds of condition:
**DERIVED (`duration 0`)** — a band on a need's satisfaction, computed from `(row, tic, corpus)` by
every observer identically, with no grant events at all (F2); **TIMED (`duration > 0`)** — stored
grants `(pawn, condition_id, grant_tic)` expiring `duration` tics later (the action stream's kind).
The word *conditional* is retired for this pair (conditions F4): it would now read as "a conditional
condition".

**Indexed slots are bare digits** (`band.0`, `needs.3`) — the `packed.<i>` shape. Safe because
these nodes never hold a scalar sibling (the `rotation_key` I8 hazard); do not add one.

## TOML content schema (`content/*.toml` → `shared` loader → every consumer)

**The corpus is DATA** (work [`2026-08-04-toml-content`](work/2026-08-04-toml-content/README.md)):
one record per def, organized by category — `tiles.toml`, `things.toml` (pawns included),
`biomes.toml`, `materials.toml`, `needs.toml`, `subtypes.toml`.

**The corpus is a TREE, and a FOLDER IS A PACKAGE**
([`2026-08-05-content-packages`](work/2026-08-05-content-packages/README.md) F1). The loader reads
`content/**/*.toml` at any depth, sorted by path relative to `content/`. Drop `content/mods/foo/` in
and its definitions are part of the world — **no manifest, no registration, no declared load
order**:

- **Ids come from the registry, not corpus position**, so file order cannot renumber anything in a
  running world. Sorting exists for determinism across machines, not for identity.
- **A collision is a LOAD ERROR by construction.** Two packages claiming one
  `(type, subType, kind, variant, version)` fails the load — never a silent last-writer-wins.
- **Server-only content is filtered by what the DATA IS, not what the file is called** (F2). The
  edge strips `[[biome]]` blocks from every source it serves and withholds a biome-only file
  entirely, so a package may name its files anything without leaking worldgen rules to clients.
- **To change an existing definition, a package authors a new VERSION of it** — which coexists with
  the original and wins name resolution by being newest. There is no override mechanism, deliberately
  ([I3](work/2026-08-05-content-packages/issues.md#i3)).

`rd content-check` enforces the one rule that remains: every tracked file under `content/`, at any
depth, is a `*.toml` a human wrote.

**The corpus DESCRIBES; the server NUMBERS** (work
[`2026-08-04-definition-registry`](work/2026-08-04-definition-registry/README.md) F1). A def authors
its **taxonomy** as names and no numbers at all:

| Field | Shape | Meaning |
|---|---|---|
| `type` | scalar name | the structural family — `biome-tile`, `biome-thing`, `pawn`, … (a code palette; types imply a pipeline) |
| `kind` | scalar name | the thing itself — `conifer`, `wolf`, `smooth` |
| `subType` | **array** of names | every subtype this def applies to — a biome for a `biome-thing`, a species for a `pawn` |
| `variant` | **array** of names | every variant it applies to — usually `[0..15]`, or a named form like `wall` |

`subType` and `variant` are **applicability arrays**, not coordinates ([F2](work/2026-08-04-definition-registry/forks.md#f2)):
the def applies to every tuple in the cross-product, so one `conifer` block covers 48 tuples rather
than 48 blocks. A def that exists in exactly one form still writes single-element arrays — one
spelling, not two. Wildcards are deliberately NOT supported: `"*"` would silently capture a subtype
added later and mint ids nobody authored.

The taxonomy is also the TEXTURE PATH: `<type>/<subType>/<kind>/<variant>` is the art tree's shape,
so the stem is derived, never authored. `texture = "white"` survives as the **no-art fill** — it is
not a taxon and no such file exists under `textures/`.

**Numbering.** At load the server expands the cross-product and allocates one `u32`
`definition_reference` per tuple into the registry table (`TABLES.md` § definitions), keyed by the
four names. **The packed layout does not change**
([F13](work/2026-08-04-definition-registry/forks.md#f13)): `type_id:4 | subtype_id:12 |
kind_id:12 | variant_id:4`, which fixes **16 variants per (type, subType, kind)** — `pawn/animal/
wolf` holds 15 of them today. The allocator refuses a 17th rather than wrapping.

**Versioning.** Change any field the SIMULATION reads and the def's `version` bumps, minting a NEW
id and leaving the old row in place ([F12](work/2026-08-04-definition-registry/forks.md#f12)); art,
tint and comments do not bump. Name resolution takes the highest version, so **new** placements get
the new definition while **existing objects keep their old id forever** and keep behaving as they
did ([F6](work/2026-08-04-definition-registry/forks.md#f6)). An old apple stays an old apple — same
weight, same expiry — until it is spent. There is no migration sweep, deliberately.

**Gameplay definitions** (work
[`2026-08-06-interactions`](work/2026-08-06-interactions/README.md) F1/F9): needs, conditions,
traits, interactions and affordances are defs of `type = "gameplay"`, numbered by the SAME
registry — their u32 `definition_reference`s ride payload words and event inputs. Their taxonomy
is DERIVED, not authored (F9): `subType` = the category table's name (`need`, `condition`,
`trait`, `interaction`, `affordance`), `kind` = the def's `name`, `variant` = `["default"]`
unless authored — the lane for later variations (e.g. an `angry` drink), with the AFFORDANCE
listing which variations it offers. Gameplay defs have no art: the taxonomy-is-texture-path rule
does not apply to `type = "gameplay"`, and no stem derives from it.

_Superseded: the explicit `id = N` law (toml-content F1) — correct while the LOADER owned identity,
wrong once a registry does. `needs.toml`'s ids were the last holdouts; interactions F1 pulls them
through the registry too._

Colours are `"#rrggbb"` strings. Fractions are `0..1`. Unstated fields keep the defaults the
`.rd` loader used (documented per table below).

```toml
# ── tiles.toml ────────────────────────────────────────────────────────────────
[[tile]]
type = "biome-tile"         # taxonomy — the id is ALLOCATED from these, never authored
kind = "grass"
subType = ["default"]       # applicability array
variant = [0]               # applicability array
name = "grass"              # the resolution name (what worldgen + the build panel ask for)
texture = "white"           # ONLY for the no-art fill; a real stem derives from the taxonomy
tint = "#4b573e"
height = 1.0                # world-z wall height (optional; 0 = flat ground)
build = "wall_smooth"       # the kind the build panel places on this tile (optional)
cast_shadow = 1             # numeric lighting MODE lane (0 = off), passed through
receives_shadows = 2        # numeric MODE (ground tiles author 2), passed through
rotation = 0                # optional; fixed rotation index
linked = { w = 4, h = 4 }   # autotile grid (optional — marks a linked kind)
padding = 0.5               # internal padding, UNITS of the 16-unit cell (linked only)
packed = [                  # up to 4 material channel bindings, index = RGBA channel
  { material = "mottle", tint = "#6b6b6b" },
]
affordances = [             # affordance bindings: the reference + THIS carrier's parameters
  { name = "drink_water", magnitude = 3 },   # (tiles and things alike; interactions F2)
]

# ── things.toml — flora, walls' kinds, PAWNS (a pawn is a thing with parts) ──
[[thing]]
type = "pawn"               # taxonomy — the id is ALLOCATED from these, never authored
kind = "wolf"
subType = ["animal"]        # applicability array
variant = ["0"]             # applicability array
name = "wolf"
speed = 12                  # TICS per tile (optional; movement default applies)
needs = ["thirst"]          # the needs this kind carries (optional)
traits = ["biological_lifeform"]   # the traits this kind carries (optional; interactions F6)
light = {                   # the kind's emitted light (optional; reach>0 = lit)
  r = 1.0, g = 0.8, b = 0.5, intensity = 1.0, reach = 16, radius = 0.25,
  height = 0.5, cast = true, hot = false, flicker = false }
packed = [ { tint = "#5f6b3c" }, { tint = "#6e4a2e" } ]   # material optional per channel

  # EVERY def's visual is a parts ARRAY; a single-sprite thing has one entry.
  # Slot index = array order (body = 0, head = 1 …).
  [[thing.part]]
  texture = "pawn/animal/wolf"   # stem; omit for a flat tint rect
  tint = "#ffffff"
  geo = "#8a8f98"                # geo-tier silhouette colour (defaults to tint)
  span = 1                       # frame world span, pow2 tiles
  scale = 0.8                    # the PART scale (a pawn slot's art scale)
  sprite_scale = { w = 0.5, h = 0.5 }   # the pre-atlas ingest pair — a SEPARATE channel
  size = 2                       # legacy drawn-box tiles (superseded by span; kept)
  anchor = { x = 0.5, y = 1.0 }          # logical anchor in the footprint
  sprite_anchor = { x = 0.5, y = 1.0 }   # pivot on the subframe
  part = 1                       # master file part suffix (head = 1; default 0)
  depth = 0.1                    # per-slot draw depth (negated facing away)
  offset = { z = 0.6 }           # elevation offset, tiles

    # Subframes: keys mirror the resolver's fallback chain EXACTLY —
    # `default` → per-rotation (`s`/`e`/`n`/`w` alias r0..r3, or `r4`..`r15`) →
    # per-variant (`v0`..`v15`) → fully-specific (`v3.e`). Values are fractions;
    # `ax`/`ay` optional (pivot per rect).
    [thing.part.subframe]
    default = { x = 0.2188, y = 0.0391, w = 0.5625, h = 0.9336, ay = 1.0 }
    e  = { x = 0.05, y = 0.26, w = 0.90, h = 0.47 }
    v0 = { x = 0.3359, y = 0.2109, w = 0.3359, h = 0.5898 }

# ── biomes.toml — the classifier as data (F3); array order = evaluation priority ──
[[biome]]
name = "wetland"
subtype = 4                 # the stored subtype_id (explicit since the DSL days)
# every listed dimension must pass (conjunction — the corpus never used `or`);
# keys mirror the retired ops EXACTLY: gte (≥), lt (<), lte (≤), gt (>)
when = { humidity = { gte = 0.60 }, elevation = { lt = 0.46 } }
tile = "dirt"
scatter = [                 # ordered, least→most dominant; LAST hit wins the cell
  { salt = 3, p = 0.25, thing = "reed" },
]
# dimensions: 0 = temperature, 1 = humidity, 2 = elevation (named keys map by
# this fixed order); the per-tile draw is SplitMix64(seed ^ salt·φ) — the exact
# `^rand` derivation, inherited so no scatter re-rolls.

# ── materials.toml ────────────────────────────────────────────────────────────
[[material]]
id = 1
name = "mottle"
noise_field = "mottle"      # strand|mottle|speckle|vein|grain|clump
hue_swing = 10.0            # degrees at full noise
chroma_swing = 0.03
warm_cool_bias = 0.2        # −1..1 cool..warm
sample_space = "world"      # "uv" (default) | "world"
detail = { field = "grain", amp = 0.4, scale = 1.0 }   # normal detail (optional)

# ── gameplay defs — needs, conditions, traits, interactions, affordances ─────
# (needs.toml, interactions.toml — ANY file; the category table is what matters.)
# NO ids and NO authored taxonomy (F9): the tuple derives as
# `gameplay/<category>/<name>/default`, and the registry numbers it like any def.
[[need]]
name = "thirst"
label = "Thirst"
min = 0                     # the authored value domain (f32; interactions F7) —
max = 100                   # min can be negative: min/max IS the sign treatment
deplete = 21600             # TICS max→min; 0/absent = never drains
band = [                    # exclusive ranges in the need's OWN units; ≤1 active
  { condition = "thirsty",    lo = 10, hi = 35 },
  { condition = "dehydrated", lo = 0,  hi = 10 },
]

[[condition]]
name = "thirsty"
label = "Thirsty"
mood = -0.15                # offset while active; mood = clamp(0.5 + Σ)
duration = 0                # TICS a TIMED grant lives; 0 = DERIVED (band-computed)
priority = 0                # card sort key, desc; absent = 0

[[trait]]                   # a capability class a pawn HAS (kinds author `traits = [...]`)
name = "biological_lifeform"
label = "Biological Lifeform"

[[interaction]]             # something a pawn can DO (interactions F5)
name = "drink"
label = "Drink"
inputs = ["pawn", "need", "amount"]   # the SIGNATURE — event inputs bind these IN ORDER
# effects: operands are `"@input"` references or constants; `amount` is signed f32,
# clamped to the need's authored min/max on apply
satisfy = { target = "@pawn", need = "@need", amount = "@amount" }
grant = ["quenched"]        # TIMED condition grants on execute (expiry from the condition)
location = "on"             # this turn's only rule: the target stands ON a carrier tile (F8)
duration = 0                # reserved — interactions are instantaneous (interactions I9)

[[affordance]]              # WHO may do WHAT (availability; interactions F2)
name = "drink_water"
requires = ["biological_lifeform"]    # trait gate
interaction = "drink"
variants = ["default"]      # which variations of the interaction this affordance offers
# carriers bind their parameters where they are defined (tile/thing blocks above):
#   affordances = [{ name = "drink_water", magnitude = 3 }]
# worked example: drinking at the water tile executes drink with amount = +3.0 —
# satisfaction moves from `satisfaction_at(now)` to `clamp(sat + 3.0, 0, 100)` on
# thirst's authored domain, and `quenched` is granted at the composing tic.
```

## Removed

`valid_at`, `cold_reference`, `hot_reference`, `reference_id`, `event_word` — see
[`notes/variables.md`](notes/variables.md).

**2026-07-31, the lighting strip** — the unified data texture and every band on it:
`definition_data`, `prim_data`, `billboard_data`, `light_data`, `light_presence_lo`,
`light_presence_hi`, `prim_presence`; the `shadow-cold` and `shadow_dirty` textile maps; and the
`light_presence_cold` map. Retired together because their only reader and writer, `shadowGather.ts`
+ `coldShadowData.ts`, were deleted. Recoverable from git.


