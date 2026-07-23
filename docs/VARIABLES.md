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
for free. Fixed slots: **`N = 128` lights**, `u16` def/prim indexes (⇒ 65 536 ceilings). Casters are found via
**per-tile buckets** (the per-light LUT is retired); a caster's position + definition live in **one** place
(`prim_data`). The per-tile maps live on the shared toroidal TILE grid (`cols × rows` from the cold cache
window; textile resolutions per the lighting-rebuild map-model). Work streams:
[`work/2026-07-21-shadow-bitfield`](work/2026-07-21-shadow-bitfield/README.md),
[`work/2026-07-22-lighting-rebuild`](work/2026-07-22-lighting-rebuild/README.md).

**Unit.** Every world-space quantity here (the sub-tile `anchor`, `z`, `reach`, opaque `prim_width/height`, the
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

**`light_data`** — a **128×1** `RGBA32UI` texture; **column `x` = light `x`'s record**. Written on light change:

```
R  u32 position_anchor_reference
G  u32 colour        u8 r (24–31) | u8 g (16–23) | u8 b (8–15) | u8 intensity (0–7)
B  u32 reach         u8 z (24–31, units) | u12 reach (12–23, units; ~16-zone max) | u11 reserved (1–11) | u1 cast_shadows (0)
A  u32 emitter       u8 emitter_radius (24–31, units — penumbra softness) | u24 reserved
```

**`prim_data`** — a **256×128** `RGBA32UI` texture, **2 prims/px** (`u64` each) → 65 536 slots; **index 0 is a
sentinel** (empty / end-of-list), so usable prim indices are **1..65 535**. One entry per **placed** caster
instance — the single place a prim's position + definition live (move → one texel). The position is the prim's
**true game anchor** (full-box base-centre) — the def's `offset` shifts the shadow, never the prim:

```
entry (2 per RGBA32UI px = 64 bits each)
  u32 position_anchor_reference
  u32 orient    u8 z (24–31, units) | u2 rotation (22–23, 0=S 1=E 2=N 3=W) | u16 definition_index (6–21) | u6 reserved (0–5)
```

**`prim_definition_data`** — a **256×256** `RGBA32UI` texture, one px per sprite **variant** (generic; shared by
every instance), keyed by `u16 definition_index`. Compare-written whenever its inputs change (surface decode,
LOD upgrade — a write re-dirties the gather). The model (work
[`2026-07-23-def-frame-anchors`](work/2026-07-23-def-frame-anchors/README.md)): frames are **pow2 squares** on
a 16-px atlas grid, addressed by a **lod exponent**; the **minimum bbox lives in world units** (even, so half-
anchor shifts are integral) at an unsigned frame-relative offset; a **px nudge** aligns the sampled window to
the opaque pixels (x centered, y bottom-aligned — shadows anchor at the base); **3×3 frame anchors** place the
bbox against the prim's position:

```
R  u32   u9 prim_width (23–31, 2-unit steps) | u9 prim_height (14–22, 2-unit steps)
         | u4 frame_span (10–13, tiles − 1; width = log2(ZONE_DIM)) | u10 reserved (0–9)
G  u32   u10 offset_x (22–31, units) | u10 offset_y (12–21, units) | u12 reserved (0–11)
         (UNSIGNED, frame-relative: the bbox top-left indexed into the frame — no bias)
B  u32   u10 frame_x (22–31, 16-px grid) | u10 frame_y (12–21, 16-px grid) | u4 frame_page (8–11)
         | u4 frame_lod (4–7, side = 2^lod) | u2 frame_anchor_x (2–3) | u2 frame_anchor_y (0–1)   (FULL)
A  u32   u12 nudge_x (20–31, px, signed +2048) | u12 nudge_y (8–19, px, signed +2048)
         | u2 nudge_anchor_x (6–7) | u2 nudge_anchor_y (4–5) | u4 reserved (0–3)   (FULL)
```

**The whole-px-per-unit invariant.** Minimum texture size is **1 px/unit** → smallest frame 16×16 (lod 4);
`ppu = 2^frame_lod / (16 · frame_span_tiles)` **doubles** per lod and is a whole pow2 ≥ 1 by construction
(pow2 frame over a pow2-tile span) — never fractional. `frame_lod < 4` ⇒ no silhouette resolved → solid quad.

**Field sizing.**
- `prim_width/height` — **u9 in 2-unit steps** (even bbox ⟹ integral half-anchors): ≤1022 units ≈ 4-zone
  headroom over the 256-unit (one-zone) per-prim ceiling.
- `frame_span` — **u4 = span tiles − 1**, capped at one zone; the width is `log2(ZONE_DIM)` so bumping the
  zone size drags the field with it. Valid values are pow2 tiles (1/2/4/8/16) — required for integer ppu;
  the writer rounds up + warns. Needed because lod alone can't give ppu (a 32px frame is 1-tile@2ppu OR
  2-tile@1ppu).
- `frame_x/y` — **u10 in 16-px grid**: the pow2 frame's ORIGIN on the page; 16384 GL max / 16 min frame =
  1024 grid. 16-aligned by construction (per-LOD-size pools pack uniform pow2 ≥16 squares, zero padding).
- `frame_lod` — **u4** side exponent (4..14): replaces a w/h pair — frames are square pow2, one exponent.
- `frame_anchor_x/y` — **u2**: 0 none | 1 half | 2 full shift of the bbox against the prim position
  (3×3 = TL..BR). Shadow casters use bottom-center (1, 2); rotation=W mirrors anchor_x. The gather applies
  `bbox_anchored − fullbox_anchored` so `prim_data`'s base-centre position keeps working (F3: reported-x/y
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

**`caster_buckets`** — a **`cols×rows`** `RGBA32UI` **textile_tile map**: per tile **≤ 8 casters** as `8× u16`
`prim_data` indices (same slot packing as presence; `0` = empty). A caster is bucketed into every tile its
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
