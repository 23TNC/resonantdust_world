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
at lod 0** and **`2^k × 2^k` tiles at lod k**, so the texture is constant while the world it covers grows 4×
per step. `SQUARE = 128`; **maximum art size is 128 px** — a slot cannot show more.

```
SLOTS_X = 32   SLOTS_Y = 16      VISIBLE_X = 28   VISIBLE_Y = 12   OVERSCAN = 2
LOD_LEVELS = 3                   (lod 0..2 — fits u2, with room to restore lod 3)
REFERENCE = 3584 × 1536          (the visible slots at lod 0)
```

**`SLOTS` is a POWER OF TWO on both axes, deliberately.** The toroidal wrap is `mod(wc, SLOTS << lod)`,
which stays pow2 at every lod, so it compiles to a bitmask rather than an integer division. Correctness does
not depend on it — a slot subdivides into `2^lod` tiles at any grid size — this is purely a cost property.

**MODULUS vs STRIDE.** The `textile_square` TEXTURE is `SLOTS + 2` slots per axis (34×18 = 4352×2304). The
extra ring is the **wrap-apron**, applied as a `(sx + 1)` offset AFTER the modulus, so it never enters the
wrap arithmetic: the toroidal window straddles the texture edge, and `bakeSquare` mirrors an edge slot to
the opposite border so a square adjacent across the wrap has physically adjacent texels. A non-pow2 texture
costs nothing (WebGL2 handles NPOT at NEAREST/CLAMP); the address math stays pow2 regardless. The
`textile_unit` and `textile_tile` maps carry no apron.

**One grid serves every map; only texels-per-slot differs** — which is why `lod` can be published once in
the constants px and read by every shader, instead of each deriving a slot address from a world coord (the
recurring failure [`work/2026-07-24-map-compatibility`](work/2026-07-24-map-compatibility/README.md) exists
to police).

| family | texels/slot | texture | maps |
|---|---|---|---|
| `TEXTILE_SQUARE` (per px at lod 0) | `SQUARE` = 128 | 4352 × 2304 (incl. apron) | albedo, normal, surface, zdepth |
| `TEXTILE_SQUARE`, no apron | 128 | 4096 × 2048 | lightmap |
| `TEXTILE_UNIT` (per unit) | 16 | 512 × 256 | shadow |
| `TEXTILE_TILE` (per tile) | 1 | 32 × 16 | presence, caster buckets, dirty |

**Fit is COVER, not contain:** `s = max(W / 3584, H / 1536)`. `max` (not `min`) is what keeps the viewport
entirely inside the visible slots; `min` would fit the whole grid and expose overscan at the edges.

| lod | tiles/slot | tile texels | visible tiles | zoom band (s=1) |
|---|---|---|---|---|
| 0 | 1×1 | 128 | 28×12 | [1, 2) |
| 1 | 2×2 | 64 | 56×24 | [0.5, 1) |
| 2 | 4×4 | 32 | 112×48 | [0.25, 0.5) |

`ZOOM_MIN = 0.25` stops the ladder at lod 2. Lod 3 (8×8 tiles/slot, 224×96 visible) is expressible — the
`u2` field has room and no layout changes — but its window is ~450 zones, which needs a **loading-priority
system** before it is pleasant. Deliberately deferred, not designed out.

At a 16:9 display the VERTICAL axis binds (`1440/1536 > 2560/3584`), so a 2560×1440 player sees 120-px tiles
and 21.3×12 of the 28×12 visible slots — the horizontal slack is cached-but-offscreen, which is the correct
axis to spend it on since the vertical budget is the scarcer one.

**Lod does not affect display sharpness.** Texels per screen pixel is `(128/2^k) / (σ · 128/2^k) = 1/σ` —
the `2^k` cancels. Lod selects *world coverage* only; sharpness is governed by the viewport against the
2560×1536 reference. The **bake** stage is separate and always exactly 1:1 (art mip `128/2^k` px into a
`128/2^k` texel footprint).

**Rescaling on a lod change is NEAREST** — replication up, decimation down, never averaging. Independently
mandatory for three maps, which is what permits one shared reproject path: the shadow bitfield cannot be
filtered at all, `zdepth` encodes a discrete `0x80 | baseRow` that averaging silently corrupts, and the
lightmap must stay exactly-representable to remain invertible. Linear is permitted for albedo/normal/surface
only.

## Cold shadow data textures (`client/webgl`)

The GPU shadow **gather** reads its **static** (cold) light + caster data from **ONE unified** `RGBA32UI`
**data texture** (`texelFetch`, exact integer reads), read every frame for free. Fixed slots: **`N = 128`
lights** (the shadow-cold bitfield cap), `u16` record ids (⇒ 65 536 ceilings, **id 0 reserved** as the
global sentinel). Casters are found via **per-tile buckets** (the per-light LUT is retired); a billboard's
resolved position + definition live in **one** place (the `billboard_data` band). The per-tile maps live on the shared toroidal TILE grid (`cols × rows` from the cold cache
window; textile resolutions per the lighting-rebuild map-model). Work streams:
[`work/2026-07-21-shadow-bitfield`](work/2026-07-21-shadow-bitfield/README.md),
[`work/2026-07-22-lighting-rebuild`](work/2026-07-22-lighting-rebuild/README.md),
[`work/2026-07-23-unified-data`](work/2026-07-23-unified-data/README.md).

**The unified data texture** — **1024×1024 `RGBA32UI`** (16 MB, provisioned once), linear index
`i → (i & 1023, i >> 10)`, laid out in **64-row bands** (65 536 one-texel slots each):

The texture is **16 u16-addressable SETS** (1024×64 each; `set = linear >> 16`, in-set id = the low u16):

```
set 0   rows   0–63    definition_data        1 px per def
set 1   rows  64–127   prim_data              1 px per placed PRIM (composition node)
set 2   rows 128–191   light_data             1 px per light record
set 3   rows 192–255   light_presence_lo      1 px per TILE (region-torus fold) — light slots 0–7
set 4   rows 256–319   billboard_presence     1 px per TILE (region-torus fold) — 8 billboard slots
set 5   rows 320–383   light_presence_hi      1 px per TILE (region-torus fold) — light slots 8–15
set 6   rows 384–447   billboard_data         1 px per billboard record
sets 7–14              reserved               (materials-era tables)
        (set 0 is the SENTINEL in a prim's set_a..d — "no data carried"; and in-set
         id 0 is the GLOBAL SENTINEL so commands can pad with zeros)
set 15  row  1023 tail constants px (in-set id 64 512):
        R  u16 id | u16 cols        G  u16 rows | u2 lod (14–15) | u14 slot (TEXTILE_UNIT)
        B  i16 winCol | i16 winRow  A  u16 light_count | u16 tilt_centideg
          (tilt_centideg = world ground tilt × 100, e.g. 65.00° → 6500; every lighting shader reads
           the live angle from here instead of a compile-time literal or a dedicated uniform)
          (cols/rows are SLOT counts — 24×16 — not tile counts, since the slot grid is fixed and the
           tiles it covers vary with lod: tiles = cols · 2^lod. `lod` is published HERE so every shader
           reads ONE authoritative mapping; texels-per-slot stays a per-family compile-time constant
           (SQUARE / TEXTILE_UNIT / 1). `slot` narrowed u16→u14 to make room — it holds 16.)
```

**Naming — a PRIMITIVE presents as a BILLBOARD or a LIGHT.** A *primitive* is a DSL-declared world
object (delivered through the cache as `Primitive`). It can present in more than one way: as a
**billboard** (a view-plane sprite — trees, pawns; the shadow *caster* + lit surface) or as a **light**
(an emitter). The data-texture records are per-**presentation**: `billboard_definition_data` +
`billboard_data` hold the billboard presentation (atlas-frame def + placed record); `light_data` holds
the light presentation. (Renamed 2026-07-24 from `prim_*` — "prim" ambiguously meant the billboard;
"primitive" is now the umbrella. See [`work/2026-07-24-light-prims`](work/2026-07-24-light-prims/README.md).)

**DATA holds PERSISTENT state only** — a map lives here iff it survives across frames and changes on
events (object created/moved/destroyed, zone streamed/evicted). `shadow_dirty` (rebuilt per frame)
and the `shadow-cold` RT (GPU-written) are **NOT** in DATA — folding a per-frame map in would mean
re-scattering ephemeral state every frame, worse than the `texSubImage` it replaces.

**Every record SELF-ADDRESSES: its u16 in-set id lives in R's high half.** That makes scatter
commands PURE PAYLOADS (v2.1 — the scatter reads the target out of the record itself).

**Region-torus tile addressing** (`light_presence` / `billboard_presence`, work
[`2026-07-23-presence-in-data`](work/2026-07-23-presence-in-data/README.md)). A tile-keyed set is
**one region's tiles** (`REGION_DIM=16` zones × `ZONE_DIM=16` tiles, squared = 65 536 = one set),
addressed by **in-region** coordinates with NO region bits — valid because the viewport window is
≤ one region, so two zones sharing a residue (256 tiles apart) can never be visible together. The
in-set id is the **zone-strip fold** (a zone's 256 tiles contiguous in one row):

```
world tile (wc, wr):
  zx = pmod(floor(wc/16), 16)   zy = pmod(floor(wr/16), 16)   // in-region zone
  tx = pmod(wc, 16)             ty = pmod(wr, 16)             // in-zone tile
  in_set_id = ((zx>>2) + zy*4) * 1024  +  (zx&3)*256 + ty*16 + tx
```

`light_presence_*` / `billboard_presence` px: `R = slot0|slot1`, `G = slot2|slot3`,
`B = slot4|slot5`, `A = slot6|slot7` — **8 slots/tile**. (Retiring self-addressing freed the u16 the
in-set id occupied, which is what buys back the 8th slot.) Slots are `u16` record ids with **`0` =
empty**, the same global sentinel commands pad with. Presence spans **two sets** (`_lo` slots 0–7,
`_hi` slots 8–15) → **16 lights/tile**; `billboard_presence` gives **8 billboards/tile**. Both carry
**leaf** ids, never carrier prims — bucketing carriers would make the per-tile billboard count
unbounded (a carrier fans out to ≤4, recursively).

**The two maps are NOT the same relation, and that is deliberate.** A **light knows exactly which tiles
it affects**, so it writes its id into every tile within reach — `light_presence` is a **reach** relation,
and a light found there may sit up to `reach` tiles away (hence `resolved_zone`, above). A billboard
cannot do the same for its *shadow*: shadows are **combinatorial** (every billboard × every light that
reaches it), so their count is indefinite and storing shadow presence is impossible. Instead
`billboard_presence` is a **containment** relation — a billboard is registered only on the tiles it
itself occupies, so finding it there means you found **its** tile, and the gather **projects the shadow
from there** during the corridor walk. That is why a billboard needs no `resolved_zone` and a light does.

**`shadow-cold`** (the gather OUTPUT RT, world-space toroidal `RGBA32UI`, textile_unit) packs **14 ×
u9 coverage** (512 levels) with NO channel straddle: the **low 8 bits** of slot `i` sit at channel
`i>>2`, bits `(i&3)·8` (R = slots 0–3, G = 4–7, B = 8–11, A = slots 12–13 in bits 0–15); the **9th
(high) bit** of slot `i` sits in **A bit `16+i`** (A bits 16–29 = the 14 high bits; bits 30–31
reserved). So `value_i = low8_i | (high1_i << 8)`, `0..511` = **126 bits used**. Slot `i` of the
shadow matches slot `i` of presence (same light). u9 is the coverage resolution emitter-based
penumbra gradients need (work [`2026-07-23-penumbra`](work/2026-07-23-penumbra/README.md)).

Updates arrive by **command-buffer scatter**: a **64×64 `RGBA32UI` command buffer** uploaded as one
contiguous row-span (rotating row cursor — never overwrite just-consumed rows) and applied by one
point-scatter draw per fill. Commands are **absolute whole-texel writes → replay-idempotent** (no
clearing; the draw count is the only cursor):

**Command format v3** (work [`2026-07-25-primitive-graph`](work/2026-07-25-primitive-graph/README.md)) —
records no longer self-address, so the **command carries the target ids**. Every command is exactly
**8 px (128 B)**, so 8 commands tile a 64-px row = **56 record-writes/row**:

```
command = 8 px, fixed stride (command k begins at px k·8)
  px 0  header   R: u8 operation (24–31) | u5 set (19–23) | u3 count (16–18) | u16 id₀ (0–15)
                 G: u16 id₁ | u16 id₂    B: u16 id₃ | u16 id₄    A: u16 id₅ | u16 id₆
  px 1–7         up to 7 payload records, written to (set, id₀ .. id_{count−1})
  operation 0x01 = write-data; the operation DEFINES the rest of the command, so further
  8-px operations (presence writes, bulk clears) take their own codes
```

**`count` (u3) states how many of the 7 ids are live**, so a partial command needs no sentinel and
**`id = 0` stays a fully usable record id** — which is required, because the tile-keyed sets
(`light_presence_*`, `billboard_presence`) address by **`foldTile`**, whose range is
**0..65535 exhaustively**: fold `0` is a real tile (zone 0,0 / tile 0,0) and there is no spare id to
bias into. `set` is **u5** (32 sets; 16 in use) — widen it out of `operation` later if ever needed.

The scatter issues **7 points per command** and a point with `slot ≥ count` emits an off-clip position
(never rasterised), so the index map stays trivial — record `p` → command `p/7`, slot `p%7`, payload px
`(p/7)·8 + 1 + p%7` — and the scatter vertex **drops its per-set count scan** entirely.

**Unit.** Every world-space quantity here (the sub-tile `anchor`, `z`, `reach`, opaque `billboard_width/height`, the
anchor `offset`) is in **units**, a compile-time constant `1 unit = SQUARE/16 = 4px` (`TILE = 16 units`,
derivable — not stored). The shader works in units; no per-frame scale uniform. The **only** exception is the
silhouette `frame_*` fields, which are **texture pixels** (they index the atlas page, a different space).

**Shared position — full spatial address + sub-tile anchor** (extends `position_reference`: the low byte is a
sub-tile `anchor` instead of a `layer`, giving `SQUARE/16` = 1-unit resolution; `anchor = (8,8)` centres a prim):

```
u32 position_anchor_reference       region | zone | tile | anchor  (min unit = SQUARE/16 px)
  u8 region_reference               bits 24–31    x:4 | y:4
  u8 zone_reference                 bits 16–23    x:4 | y:4
  u8 tile_reference                 bits 8–15     x:4 | y:4
  u8 anchor_reference               bits 0–7      anchor_x:4 | anchor_y:4   (0..15 within the tile)
```

### The primitive graph — a prim CARRIES presentations
Work [`2026-07-25-primitive-graph`](work/2026-07-25-primitive-graph/README.md). A **primitive** is a
DSL-declared world object. A **`prim_data`** record is a *composition node*: positioned, carrying **up to
4 pieces of data** (`set_a..d` names the band, `id_a..d` the record; **set 0 = "no data"**). A piece may be
a **`billboard_data`** (sprite presentation), a **`light_data`** (emitter presentation), or **another
`prim_data`** — so a torch = prim{billboard, light}, a pawn = prim{head, body, hand-prim, hand-prim}, and a
hand = prim{billboard, tool-prim}. **Records no longer self-address** — the command carries the ids.

Inheritance is **downward-forcing**: topmost `hot_cold` **hot** wins; topmost **`!cast_shadows`** wins.
**One object per layer**, so a prim's carried pieces occupy distinct layers (`prim_data` holds no layer).
All offsets are **bias-8 signed** per nibble (stored − 8 ⇒ −8..+7), so children place in any direction.

**`prim_data` band** (rows 64–127) — 1 px per placed prim. `child` reinterprets RED: a **root** is placed
absolutely; a **child** points at its carrier and holds only offsets.

```
R  u32   ROOT  (child=0)   u8 region_reference (24–31) | u8 zone_reference (16–23)
                         | u8 tile_reference (8–15)   | u8 unit_reference (0–7)
         CHILD (child=1)   u16 parent_id (16–31) | u8 tile_offset (8–15) | u8 unit_offset (0–7)
G  u32   u2 reserved (30–31) | u1 inherit_rotation (29) | u1 child (28) | u2 rotation (26–27)
         | u1 hot_cold (25) | u1 cast_shadows (24) | u8 z (16–23, units)
         | u4 set_a (12–15) | u4 set_b (8–11) | u4 set_c (4–7) | u4 set_d (0–3)
B  u32   u16 id_a (16–31) | u16 id_b (0–15)
A  u32   u16 id_c (16–31) | u16 id_d (0–15)
```

**`billboard_data` band** (rows 384–447) — 1 px per billboard. RED is **parent + the CPU-resolved
position**; GREEN keeps the **authored** offsets (the durable relative placement).

```
R  u32   u16 parent_id (16–31) | u8 resolved_tile (8–15) | u8 resolved_unit (0–7)
G  u32   u4 layer (28–31) | u2 rotation (26–27) | u1 hot_cold (25) | u1 cast_shadows (24)
         | u8 z_offset (16–23) | u8 tile_offset (8–15) | u8 unit_offset (0–7)
B  u32   u16 definition_id (16–31) | u2 last_lod (14–15) | u6 reserved (8–13) | u8 seed (0–7)
A  u32   u32 reserved
```
`last_lod` — the lod this billboard was **last baked at**. A mismatch against the live lod means the def is
stale and wants swapping; the swap rides the existing billboard dirty cascade. "Last" is correct here
because billboards are **re-baked, not accumulated** — there is nothing to invert, so no history is needed.
Contrast `light_data.coarsest_lod`, which is a different question with a different answer.

`seed` (2026-07-27, [material-system](work/2026-07-27-material-system/README.md) P2) — the object's
material-variance seed, `cellSeed(tx, ty)` quantised to u8: DETERMINISTIC from the world cell, so two
adjacent same-kind objects differ while each is pinned to where it stands across reloads. The CPU
stamps it at record write and the bake consumes the same value (one source); it lives in the record
so any future GPU consumer reads the identical seed.

No `resolved_zone`: `billboard_presence` is a **containment** relation (a billboard is bucketed into the
tiles its footprint covers), so the fragment's own tile pins it — nearest-congruent is exact for any
footprint under 8 tiles.

**`light_data` band** (rows 128–191) — 1 px per light. Props are **inline** (there is no light-def band).

```
R  u32   u16 parent_id (16–31) | u8 resolved_tile (8–15) | u8 resolved_unit (0–7)
G  u32   u4 layer (28–31) | u2 rotation (26–27) | u1 hot_cold (25) | u1 cast_shadows (24)
         | u8 z_offset (16–23) | u8 tile_offset (8–15) | u8 unit_offset (0–7)
B  u32   u8 r (24–31) | u8 g (16–23) | u8 b (8–15) | u8 intensity (0–7)
A  u32   u12 reach (20–31, units) | u8 emitter_radius (12–19, units)
         | u8 resolved_zone (4–11) | u2 coarsest_lod (2–3) | u2 reserved (0–1)
```
**`coarsest_lod` is the COARSEST lod since this light was last cast — not the last lod.** The distinction is
load-bearing for the invertible lightmap: a **down-then-up round trip is lossy**. Decimating 32→16 destroys
three of every four texels; replicating back to 32 leaves texel `(a,b)` holding `v(2·(a>>1), 2·(b>>1))`, not
`v(a,b)`. With `last_lod` the subtract would evaluate at `(a,b)` and the difference **leaks light
permanently**. Information is only ever destroyed by decimation and never returns, so the stored contribution
always lives on the coarsest grid the light has passed through — evaluate there and it cancels exactly.

Maintained as a **monotone max** (`coarsest = max(coarsest, new_lod)` on every lod change), reset to the live
lod whenever the light is genuinely re-cast. Because it is **per light, not per tile**, a tile may hold
contributions deposited at all four lods at once and a zoom costs nothing at update time — each light
independently knows the grid it must be undone on. Monotonicity also means nothing restores resolution on its
own: a light zoomed out and back in stays coarse until the refinement queue re-casts it.
**`resolved_zone` is required here** (unlike a billboard): `light_presence` is a **reach** relation, so a
light sits up to `reach` tiles from the fragment reading it. `resolved_tile` pins the light only modulo
**16 tiles**, which is ambiguous past **±8** — and `LIGHT_REACH` is **12 tiles** (the `u12` field allows
255). With zone the period becomes **256 tiles**; region stays inferred by nearest-congruent, exactly as
the region-torus fold already does.

**Resolution.** `resolved_*` is the **CPU-computed absolute position**: start at the root's
`tile`/`unit`, apply each child prim's offsets down the chain, stamp the leaf. The GPU reads only the
resolved fields, so it **never walks the graph** in the gather's per-texel-per-light loop. `parent_id`
exists for the CPU (dirty cascade, subtree free, rotation reconciliation) and permits a bounded GPU walk
if one is ever needed.

**Rotation is a reconciliation signal, not a render input.** The shader always uses the **definition's**
rotation, because the definition *is* the active sprite. A record's `rotation` is the **desired facing**;
when it disagrees with the active definition's, the **CPU swaps the definition** (until then the old
sprite renders — by design). `inherit_rotation` lives on **`definition_data`** (all objects of a kind
behave alike) with a **`prim_data`** override for an independent prim, and inherits **one step** — a
child takes its parent's rotation, never a chain walk.

**`definition_data` band** (rows 0–63) — **one px per ATLAS FRAME**: defs are
**IMMUTABLE**, keyed `(stem, cell, lod)` — a new lod landing mints a NEW def, and `billboard_data` keeps whatever
def it holds until the writer swaps its `definition_id`, which rides the **billboard dirty cascade**
(billboard tiles → lights reaching them → those lights' cast regions). Nothing ever rewrites a def, so texture/def
changes cost exactly a prim update. The lod-0 def is the LOOSE fallback (full-footprint box, solid quad).
The model (work
[`2026-07-23-def-frame-anchors`](work/2026-07-23-def-frame-anchors/README.md)): frames are **pow2 squares** on
a 16-px atlas grid, addressed by a **lod exponent**; the **minimum bbox lives in world units** (even, so half-
anchor shifts are integral) at an unsigned frame-relative offset; a **px nudge** aligns the sampled window to
the opaque pixels (x centered, y bottom-aligned — shadows anchor at the base); **3×3 frame anchors** place the
bbox against the prim's position:

```
R  u32   u4 layer (28–31) | u2 rotation (26–27) | u1 inherit_rotation (25) | u1 reserved (24)
         | u10 offset_x (14–23, units) | u10 offset_y (4–13, units) | u4 type (0–3)
G  u32   u9 billboard_width (23–31, 2-unit steps) | u9 billboard_height (14–22, 2-unit steps)
         | u4 frame_span (10–13, tiles − 1; width = log2(ZONE_DIM)) | u10 reserved (0–9)
         (offsets UNSIGNED, frame-relative: the bbox top-left indexed into the frame — no bias)
B  u32   u10 frame_x (22–31, 16-px grid) | u10 frame_y (12–21, 16-px grid) | u4 frame_page (8–11)
         | u2 frame_lod (6–7, DISPLAY lod 0–3) | u2 reserved (4–5)
         | u2 frame_anchor_x (2–3) | u2 frame_anchor_y (0–1)
A  u32   u12 nudge_x (20–31, px, signed +2048) | u12 nudge_y (8–19, px, signed +2048)
         | u2 nudge_anchor_x (6–7) | u2 nudge_anchor_y (4–5) | u4 reserved (0–3)   (FULL)
```

**`frame_lod` is the DISPLAY lod (0–3), and the frame side is DERIVED.** It was a u4 *side exponent* (4..14);
on the fixed slot grid that is redundant, because a frame spanning `S` tiles at lod `k` is exactly
`S · SQUARE / 2^k` px. Side falls out of `(frame_span, frame_lod)`, so the exponent collapses to the 2-bit
ladder and frees 2 bits.

**The whole-px-per-unit invariant, now trivial.** Substituting the derived side into
`ppu = side / (16 · frame_span_tiles)` cancels the span entirely:
```
ppu = SQUARE / (16 · 2^lod)      →  lod 0: 8   lod 1: 4   lod 2: 2   lod 3: 1
```
So ppu depends on **lod alone**, is a whole pow2 ≥ 1 by construction, and the lod-3 floor is exactly the
1 px/unit minimum (a 16-px tile). The old caveat that lod alone could not determine ppu — "a 32px frame is
1-tile@2ppu OR 2-tile@1ppu" — no longer applies, because the frame side is no longer free to disagree with
the grid. `frame_span` is still needed to size the frame, but no longer to compute ppu.

**Field sizing.**
- `billboard_width/height` — **u9 in 2-unit steps** (even bbox ⟹ integral half-anchors): ≤1022 units ≈ 4-zone
  headroom over the 256-unit (one-zone) per-prim ceiling.
- `frame_span` — **u4 = span tiles − 1**, capped at one zone; the width is `log2(ZONE_DIM)` so bumping the
  zone size drags the field with it. Valid values are pow2 tiles (1/2/4/8/16) — required for integer ppu;
  the writer rounds up + warns. Needed because lod alone can't give ppu (a 32px frame is 1-tile@2ppu OR
  2-tile@1ppu).
- `frame_x/y` — **u10 in 16-px grid**: the pow2 frame's ORIGIN on the page; 16384 GL max / 16 min frame =
  1024 grid. 16-aligned by construction (per-LOD-size pools pack uniform pow2 ≥16 squares, zero padding).
- `frame_lod` — **u2** DISPLAY lod (0..3). Side is derived (`frame_span · SQUARE / 2^lod`), so the frame is
  still one square pow2 but the record no longer stores its size. Max side = 16 tiles × 128 = 2048 (lod 0),
  min = 16 (span 1, lod 3) — the whole range a 2048² page can hold.
- `frame_anchor_x/y` — **u2**: 0 none | 1 half | 2 full shift of the bbox against the prim position
  (3×3 = TL..BR). Shadow casters use bottom-center (1, 2); rotation=W mirrors anchor_x. The gather applies
  `bbox_anchored − fullbox_anchored` so `billboard_data`'s base-centre position keeps working (F3: reported-x/y
  positions land with the DSL size rework).
- `nudge_x/y` — **u12 px at the resolved lod, both signed (+2048 bias)** (rewritten on lod swap): sample
  start = `offset·ppu − nudge`. ±2048 covers 2 units × the max ppu 1024 in EITHER direction, so any
  alignment (left/center/right, top/bottom) is expressible regardless of where the opaque run sits in its
  unit. `nudge_anchor_x/y` (u2) record WHICH alignment the stored nudge encodes — default x = 1 (centered),
  y = 2 (bottom, so shadows align at the base); future nudging operations write other anchors.
- `frame_page` — **u4**: ≤16 silhouette pages (~256 MB at 2048² — beyond any sane silhouette budget; C5's
  texture array commits all layers up front). Written **0** until C5 (one shared page bound as `uSurface`;
  off-page defs fall back to solid quad).

**`light_presence_cold`** — a **`cols×rows`** `RGBA32UI` **textile_tile map** (one texel/tile, toroidal with the
window): per tile the **nearest ≤ 8 reaching lights** as `8× u16` light indices (`0xFFFF` = empty; R holds slots
0–1 high|low, G 2–3, B 4–5, A 6–7). CPU-rebuilt on light/window change. Bounds the gather to O(8) lights/texel.

**`billboard_presence`** (was `caster_buckets`) — a **`cols×rows`** `RGBA32UI` **textile_tile map**: per tile
**≤ 8 casting billboards** as `8× u16` `billboard_data` ids (same slot packing as presence; `0` = empty). A
caster is bucketed into every tile its
tilted card's ground extent spans (base row up to `0.5·cos65°·H` above). The gather's reach-walk reads these.

**`shadow_dirty`** — a **`cols×rows`** `R8UI` **textile_tile map**, **nonzero = recompute**; gates the
single-pass gather (clean tiles `discard`, so `shadow-cold` persists). Per-slot owner tracking dirties a slot
when its world tile changes (pan) or the cold data rebuilds.

**`shadow-cold`** — the **output**: a world-space **toroidal** `RGBA32UI` **textile_unit map**
(`cols·16 × rows·16`; 1 texel = 1 unit²). Per texel, **R = 8× 4-bit coverage nibbles**, one per presence SLOT
(slot `i` at bits `i·4..i·4+3`; the slot → light indirection is that tile's `light_presence_cold` texel).
Updated in place by the single-pass gather (`discard` on clean tiles → no ping-pong). G/B/A reserved.

**Notes.** (1) `rotation` (n/e/s/w) picks the shadow regime (E/W vs N/S); `3 = W` mirrors the E frame — the
gather mirrors `offset_x` and the silhouette `s` coordinate. (2) The gather is bounded per texel: ≤ 8 lights
(presence) × reach-walk of bucketed tiles × ≤ 8 casters/tile. (3) All world fields are in **units**; decode
`region|zone|tile|anchor` → units via the `*_DIM` world constants (§Where it is), then units → px (×4) only at
the clip transform. (4) `cast_shadows` (bit 0 of reach): a **0** light lights without casting — skipped in the
gather, so a fill/ambient light costs no shadow work. (5) The silhouette sample happens only **after** the
point-in-projected-quad test proves the texel occluded — the inverted `(s,t)` is in-range by construction, so
no `u/v`-out-of-range reject class exists.

## Removed

`valid_at`, `cold_reference`, `hot_reference`, `reference_id`, `event_word` — see
[`notes/variables.md`](notes/variables.md).
