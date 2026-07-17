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

16 types × 4096 subtypes × 4096 kinds × 16 variants.

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

## Cold storage

```
u32 cold_row_reference            realm-unique within its module (the module's type_id IS the shard)
  u16 macro_position_reference    bits 16–31
  u12 subtype_id                  bits  4–15
  u4  layer_id                    bits  0–3

u32 kind_pos_reference
  u16 kind_reference              bits 16–31
  u8  tile_reference              bits 8–15
  u8  data                        bits 0–7

u8 data                           things only (tiles are kind-only, no data)
  u2 rotation                     bits 6–7      4 facings; west mirrors east
  u6 count                        bits 0–5      0–63
```

Row identity = `(macro_position_reference, subtype_id, layer_id)` — the `cold_row_reference`. **The
`type_id` is the shard**: the `tile` module *is* `TYPE_BIOME_TILE`, `thing` *is* `TYPE_BIOME_THING`,
so `type_id` is out of the key **and** off the row — dropping its `u4` (plus the old `reserved:8`) is
exactly what freed the `u12 subtype_id`. A zone with N biomes is **N rows**, one per `subtype`.

Reconstruction sources `type_id` from the shard:
`type_reference` = `shard.type_id:4 | row.subtype_id:12` → `definition_reference` =
`type_reference:16 | entry.kind_reference:16`; `layer_reference` = `shard.type_id:4 | row.layer_id:4` →
`position_reference` = `row.macro_position:16 | (entry.tile_reference:8 | layer_reference:8)`.

`data` is **universal for things** — always `rotation:2 | count:6`, no per-type parse; game rules
manipulate/display `count`. Mutation never writes the cold table: a changed cell is a `state_log` row
in the shard's own hot-format overlay (`TABLES.md`), folded back into cold by GC. There is no separate
tombstone table — a removal is a `state_log`/`state` row with the removed marker.

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

## Legacy — retiring, do not build on

```
u32 zone_id                       superseded by macro_position_reference + realm_reference
  u8 realm                        bits 24–31
  u8 region                       bits 16–23
  u8 zone                         bits 8–15
  u8 reserved                     bits 0–7

u32 region_id                     = zone_id & REGION_ID_MASK (0xFFFF_0000)
  u8 realm                        bits 24–31
  u8 region                       bits 16–23
  u16 reserved                    bits 0–15

u64 thing                         superseded by kind_pos_reference + data
  u16 kind | u4 x | u4 y | u5 data | u3 layer | u5 variant | u27 reserved

u8  tile                          tile-kind; a zone's tiles are a dense Vec<u8>[256]
u8  offset                        x_off:4 | y_off:4
```

---

## Removed

`valid_at`, `cold_reference`, `hot_reference`, `reference_id`, `event_word` — see
[`notes/variables.md`](notes/variables.md).
