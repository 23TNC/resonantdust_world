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

The crop moved to atlas **ingest**, from a DSL-authored subframe
([`work/2026-08-02-subframe-ingest`](work/2026-08-02-subframe-ingest/README.md)) — see the DSL
variables below. The lane is reserved for the **anchor** (`u16 x | u16 y`, sixteenths of a unit),
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
`content/visual/things.rd` still documents), and `screen.z = tan(world_tilt) · unit.z`, which is a
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

## Sprite subframes — the atlas crop (`shared/dsl` → `client/webgl`)

**One rect, authored once, cropped at ingest, shared by all four maps.** Work stream:
[`work/2026-08-02-subframe-ingest`](work/2026-08-02-subframe-ingest/README.md).

A texture master is a **square pow2 canvas with the subject letterboxed** inside it
(`bin/art`'s normalisation). The **subframe** says which fraction of that canvas is actually art.
The atlas ingests exactly that rect out of each of the stem's four maps — albedo, normal, surface,
layers — so they are registered with each other **by construction** rather than by two derivations
agreeing.

```
&thing.subframe.x | .y | .w | .h            fractions 0..1, default (0, 0, 1, 1) = the whole frame
&thing.subframe.<e|s|n>.x | .y | .w | .h    per-DIRECTION override, falling back to the above
&thing.sprite_anchor.x | .y                 the pivot ON the subframe, default (0.5, 0.5)
&thing.sprite_anchor.<e|s|n>.x | .y         per-DIRECTION override

&prim.subframe.…  /  &prim.sprite_anchor.…  the same, PER PART SLOT (a head is its own master)
```

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

## Needs & moodlets (`shared/dsl` → shard / npc / `client/webgl`)

**The pawn's hidden state and its displayed consequences.** Work stream:
[`work/2026-08-03-needs-moodlets`](work/2026-08-03-needs-moodlets/README.md). A **need** is a 0..1
SATISFACTION depleting toward zero (F1 — one dialect for every need; bad states are LOW; the
player never sees the scalar). A **moodlet** is what displays: a label + a mood offset. Mood is
`clamp(0.5 + Σ active offsets, 0..1)` (F5).

```
<need> ::name> @define>                     registry def — 1-based need_id, append-stable
&need.label                                 display/debug label (the need itself is never shown)
&need.deplete                               TICS full→empty; 0 = never drains (unauthored)
&need.band.<0..3>.moodlet                   the <moodlet> active while lo <= satisfaction < hi
&need.band.<0..3>.lo | .hi                  the band, 0..1 — authored EXCLUSIVE, ≤1 active/need

<moodlet> ::name> @define>                  registry def — 1-based moodlet_id, append-stable
&moodlet.label                              the displayed name
&moodlet.mood                               mood offset while active, -1..1
&moodlet.duration                           TICS a STORED grant lives; 0 = conditional (derived)

&thing.needs.<0..7>                         the needs a kind carries (:data @define, sym slots)
```

Exposed as `need_params_all()` / `moodlet_params_all()` (registry order) and
`thing_needs_table()` — **stride 8** per kind of 1-based need ids, `0` = empty slot.

**Nothing ticks a need** (F4): a pawn's shard row is `(satisfaction, set_tic)`; observers compute
`satisfaction_at(tic)` and every band-crossing tic from `deplete`. **Conditional moodlets
(`duration 0`) are DERIVED** from `(row, tic, corpus)` by every observer identically — no grant
events exist for them (F2); **timed moodlets (`duration > 0`)** are stored grants
`(pawn, moodlet_id, grant_tic)` expiring `duration` tics later (the action stream's kind).

**Indexed slots are bare digits** (`band.0`, `needs.3`) — the `packed.<i>` shape. Safe because
these nodes never hold a scalar sibling (the `rotation_key` I8 hazard); do not add one.

## Removed

`valid_at`, `cold_reference`, `hot_reference`, `reference_id`, `event_word` — see
[`notes/variables.md`](notes/variables.md).

**2026-07-31, the lighting strip** — the unified data texture and every band on it:
`definition_data`, `prim_data`, `billboard_data`, `light_data`, `light_presence_lo`,
`light_presence_hi`, `prim_presence`; the `shadow-cold` and `shadow_dirty` textile maps; and the
`light_presence_cold` map. Retired together because their only reader and writer, `shadowGather.ts`
+ `coldShadowData.ts`, were deleted. Recoverable from git.


