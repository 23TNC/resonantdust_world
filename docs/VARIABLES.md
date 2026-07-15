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
| `event_reference` | an event row | `TYPE_EVENT` | `event_log.event_reference`, `state_events.events` |

**Aliases of `server_reference`** (u8, same layout, no interior):

| alias | names |
|---|---|
| `worker_reference` | the worker owning a piece of work |
| `shard_reference` | a shard |

**Aliases of `realm_server_reference`** (u16):

| alias | names | on |
|---|---|---|
| `player_shard_reference` | the shard serving a player's data | `players.player_shard_reference` |

---

## Time

```
u16 tic                           the simulation clock; WRAPS

u64 state_uid                     the (entity, tic) slot — PK of state_log + state_events
  u16 reserved                    bits 48–63
  u32 entity_reference            bits 16–47
  u16 tic                         bits 0–15
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
u64 cold_row_reference
  u28 reserved                    bits 36–63
  u16 macro_position_reference    bits 20–35
  u16 type_reference              bits 4–19
  u4  layer_id                    bits 0–3

u32 kind_pos_reference
  u16 kind_reference              bits 16–31
  u8  tile_reference              bits 8–15
  u8  data                        bits 0–7

u8 data
  u2 rotation                     bits 6–7      4 facings; west mirrors east
  u6 count                        bits 0–5      0–63
```

Row header = `(macro_position_reference, type_reference, layer_id)`; those three **are** the row's
identity. Reconstruction: `definition_reference` = row's `type_reference` + entry's
`kind_reference`; `position_reference` = row's `macro_position` + `layer_id` + `type_id` + entry's
`tile_reference`.

`cold_removed` shares `cold_row_reference` 1:1; a tombstone is one `tile_reference : u8`.

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
