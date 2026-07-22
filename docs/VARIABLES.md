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

## Cold shadow data textures (`client/webgl`)

The GPU shadow **gather** reads its **static** (cold) light + caster data from **fixed-size** `RGBA32UI` data
textures (`texelFetch`, exact integer reads) — written on change (CPU-side; no GPU ping-pong), read every frame
for free. Everything is fixed-slot: **`N = 128` lights**, **≤ 256 shadow casters/light**, `u16` def/prim
indexes (⇒ 65 536 ceilings). The LUT (light → caster indices) is folded into `light_data`'s rows; a caster's
position + definition live in **one** place (`prim_data`). Work stream:
[`work/2026-07-21-shadow-bitfield`](work/2026-07-21-shadow-bitfield/README.md).

**Unit.** Every world-space quantity here (the sub-tile `anchor`, `z`, `radius`, `prim_width/height`) is in
**units**, a compile-time constant `1 unit = SQUARE/16 = 4px` (`TILE = 16 units`, derivable — not stored). The
shader works in units; no per-frame scale uniform. The **only** exception is the atlas `frame_*` fields, which
are **texture pixels** (they index the atlas image, a different space), not units.

**Shared position — full spatial address + sub-tile anchor** (extends `position_reference`: the low byte is a
sub-tile `anchor` instead of a `layer`, giving `SQUARE/16` = 1-unit resolution; `anchor = (8,8)` centres a prim):

```
u32 position_anchor_reference       region | zone | tile | anchor  (min unit = SQUARE/16 px)
  u8 region_reference               bits 24–31    x:4 | y:4
  u8 zone_reference                 bits 16–23    x:4 | y:4
  u8 tile_reference                 bits 8–15     x:4 | y:4
  u8 anchor_reference               bits 0–7      anchor_x:4 | anchor_y:4   (0..15 within the tile)
```

**Bit convention** for the 128-bit maps (`light_presence_cold`, `shadow-cold`): channel `R`=lights 0–31,
`G`=32–63, `B`=64–95, `A`=96–127; within a channel, light `L` is bit `L & 31`.

**`light_data`** — a **128×33** `RGBA32UI` texture; **column `x` = light `x`'s entire record** (`lut_index` is
therefore **implicit** = the column). Written on light change:

```
row 0    (x, 0)  the light record:
  R  u32 position_anchor_reference
  G  u32 colour        u8 r (24–31) | u8 g (16–23) | u8 b (8–15) | u8 intensity (0–7)
  B  u32 reach         u8 z (24–31, units) | u12 radius (12–23, units; ~16-zone max) | u11 reserved (1–11) | u1 cast_shadows (0)
  A  u32 reserved      (was lut_index|lut_count — freed: index implicit, run sentinel-terminated)

rows 1–32  (x, 1..32)  the caster LUT — 256× u16 prim indexes into prim_data, dense, terminated by a 0:
  R  u16 idx[8y-8] (16–31) | u16 idx[8y-7] (0–15)
  G  u16 idx[8y-6] (16–31) | u16 idx[8y-5] (0–15)
  B  u16 idx[8y-4] (16–31) | u16 idx[8y-3] (0–15)
  A  u16 idx[8y-2] (16–31) | u16 idx[8y-1] (0–15)          (y = 1..32 → indices 0..255)
```

**`prim_data`** — a **256×128** `RGBA32UI` texture, **2 prims/px** (`u64` each) → 65 536 slots; **index 0 is a
sentinel** (empty / end-of-list), so usable prim indices are **1..65 535**. One entry per **placed** caster
instance — the single place a prim's position + definition live (move → one texel):

```
entry (2 per RGBA32UI px = 64 bits each)
  u32 position_anchor_reference
  u32 orient    u8 z (24–31, units) | u2 rotation (22–23, 0=S 1=E 2=N 3=W) | u16 definition_index (6–21) | u6 reserved (0–5)
```

**`prim_definition_data`** — a **256×256** `RGBA32UI` texture, one px per sprite **variant** (generic; shared by
every instance of the same `(type,subtype,kind,variant)`), keyed by `u16 definition_index` (0..65 535). Written
on **atlas add**:

```
R  u32   u10 prim_width (22–31, units) | u10 prim_height (12–21, units) | u10 frame_x (2–11, atlas px) | u2 reserved (0–1)
G  u32   u10 frame_width (22–31, atlas px) | u10 frame_height (12–21, atlas px) | u10 frame_y (2–11, atlas px) | u2 reserved (0–1)
B  u32   u10 frame_page (22–31) | u8 dA (14–21, units) | u8 dB (6–13, units) | u6 reserved (0–5)
A  u32   reserved   (materials etc. — later)
```

**`light_presence_cold`** — a **128×64** `RGBA32UI` texture (sized to the **max-zoom tile window + `OVERSCAN`**),
**one px per tile**; **bit `L` set = light `L` reaches this tile** (distance cull). CPU-set when a light is
added / moves / changes radius (prims never touch it). The gather reads its tile's px and iterates only the set
bits — the per-tile light cull.

**`shadow-cold`** — the **output**: a world-space **toroidal** `RGBA32UI` bitfield (windowed like a `SquareCache`
channel, `slotPx` resolution), **bit `L` set = this pixel is in light `L`'s shadow**. Updated in place by the
single-pass gather (`discard` on clean tiles → no ping-pong). Later the per-light **mask** for lighting.

**`shadow_dirty`** — an **`R8UI` 128×64** texture (max-zoom tile window + `OVERSCAN`), **nonzero = tile dirty**;
gates the single-pass gather (clean tiles `discard`). Rebuilt per frame CPU-side; a texture, not a uniform, to
keep the fragment-uniform budget free for warm/hot.

**Notes.** (1) `rotation` (n/e/s/w) picks the shadow regime (E/W vs N/S). (2) The LUT is a **dense run of `u16`
prim indexes** in `light_data` rows 1–32, **terminated by a `0`** (prim index 0 is the sentinel) or filling all
256 — no `lut_count`; `lut_index` is implicit (the light's column). (3) The gather box-culls **per caster**
(rectangle / Chebyshev) inside the loop; the **per-tile** light cull is `light_presence_cold`. (4) All world
fields are in **units** (`1 unit = SQUARE/16 = 4px`, compile-time); decode `region|zone|tile|anchor` → units via
the `*_DIM` world constants (§Where it is), then units → px (×4) only at the clip transform. `radius` is `u12`
units ≈ **16 zones** (256 tiles). Global ambient is better a **no-cull flag** than a max radius. (5)
`cast_shadows` (bit 0 of reach): a **0** light lights without casting — its LUT run is empty (first slot = the
`0` sentinel) and it is skipped in the gather, so a fill/ambient light costs no shadow work.

## Removed

`valid_at`, `cold_reference`, `hot_reference`, `reference_id`, `event_word` — see
[`notes/variables.md`](notes/variables.md).
