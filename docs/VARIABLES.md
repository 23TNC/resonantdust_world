# Variables — the cross-component reference

> **AUTHORITATIVE** for every cross-component variable: its **name, width, and bit layout**.
> Established 2026-07-15. Where anything disagrees with this file — another doc, a comment, or the
> code itself — **this file wins and the other is the bug**.

_Scope. This file owns the **shapes**. It does not own the **reasoning**: why a shape is what it is,
what the uniqueness rules are, and where the cold/hot boundary falls live in
[`components/shared/codec/design/reference-model.md`](components/shared/codec/design/reference-model.md),
which is authoritative for exactly that and defers to this file for bits. The implementation is
`shared/codec/src/{object,refs,event_word,packed}.rs` — it is the code **of record**, not the source
of truth; when it drifts from this file, conform the code._

**Why this is the authority.** These names cross every component — the codec mints them, the modules
store them, the edge routes on them, the DSL packs them into words, the client decodes them. A name
or a nibble order that drifts between any two of those is a live bug, and a silent one. One file, one
answer.

**Changing a layout.** Edit here first, then conform the code and any doc that reproduces it. A
layout change is a wire/storage break: everything that packs or unpacks it has to move together.

**Reading convention.** Indentation = decomposition; children are listed **high bits first**, so
the order you read them down the page is the order they sit in the word, left to right. A width on
a parent is the sum of its children. Bit ranges are given where a layout is easy to get backwards.

**Verified against the code** at `shared/codec` on 2026-07-15 — every bit range below was checked
against its shift constant, not transcribed.

---

## Primitives

Every geographic `u8` is the same nibble pair — `pack_tile_reference(x, y)` is the one primitive
behind all of them (`ref_hi` = x, `ref_lo` = y).

```
u8 <spatial>_reference          ── realm | region | zone | tile all use this shape
  u4 x                            bits 4–7
  u4 y                            bits 0–3
```

Grids are 16×16 at every level (`ZONE_DIM` = `REGION_DIM` = `REALM_DIM` = 16; `ZONE_TILES` = 256).

---

## `definition_reference : u32` — *what it is*

Shareable and position-free: every instance of `pawn/human/male.fat` has the same one.

```
u32 definition_reference
  u16 type_reference              bits 16–31
    u4  type_id                   bits 28–31   0 = reserved null
    u12 subtype_id                bits 16–27
  u16 kind_reference              bits 0–15
    u12 kind_id                   bits 4–15
    u4  variant_id                bits 0–3
```

Capacity: 16 types × 4096 subtypes × 4096 kinds × 16 variants. No `subkind` — a combined kind like
`male.fat` is one `kind_id`. No reserved headroom; the u32 is exact.

---

## `position_reference : u32` — *where it is*

```
u32 position_reference
  u16 macro_position_reference    bits 16–31   the zone-subscription + cold-row key
    u8 region_reference           bits 24–31   region within realm (x:4|y:4)
    u8 zone_reference             bits 16–23   zone within region
  u16 micro_position_reference    bits 0–15
    u8 tile_reference             bits 8–15    tile within zone (x:4|y:4)
    u8 layer_reference            bits 0–7     the tile-slot
      u4 type_id                  bits 4–7     ← type is the HIGH nibble
      u4 layer_id                 bits 0–3
```

`realm` is **not** here — it rides on `server_reference` (routing). The layout self-identifies an
object-slot (`type_id` + `layer_id`) because cold objects have no row alongside them.

**Uniqueness rule: one object per `(type, layer, tile)` within a zone** — subtype-agnostic, because
a reader holding a position never knows the subtype.

---

## Identity — the handle side

### `object_reference : u32` — the universal handle

A **tagged union**: the variants share the width, not the meaning. Disambiguated by context, or by
`reference_id` when carried inside an `entity_reference`.

| variant | width | is | `unpack`able? |
|---|---|---|---|
| `hot_reference` | u32 | a live/minted object — an **opaque per-server id**, no internal structure | no (already hot) |
| `cold_reference` | u32 | a settled object *addressed by its position* — the positional layout | **yes** → hot |
| `position_reference` | u32 | a bare location — any object has one | only if it lands on a cold object |
| `event_reference` | u32 | an event row (what `ALIAS` / `AWAIT` carry) | no |
| server | u32 | a server as an object (`reserved:16 \| server_reference:16`) | no |

> **`cold_reference` and `position_reference` are two *types* sharing one layout, not one thing.**
> A position is *a place*; a cold_reference is *the settled object there*. Never distinguished by
> their bits — only by `reference_id` or context.

> **`hot_reference` is not positional.** It is a minted handle with no decomposable interior. A hot
> object's *location* is a separate `position_reference`; the handle itself says nothing about
> where it is. (Do not give it the `position_reference` tree — that conflation is what v1 did.)

### `entity_reference : u64` — an `object_reference` made globally unique

```
u64 entity_reference
  u10 reserved                    bits 54–63   headroom
  u6  reference_id                bits 48–53   which variant the object_reference is
  u16 server_reference            bits 32–47   the realm + server qualifying the handle
    u8 realm_id                   bits 40–47   = realm_reference (x:4|y:4)
    u8 server_id                  bits 32–39
  u32 object_reference            bits 0–31    the handle (union above)
```

The **low 48 bits are identical to the DSL word's qualified reference** — a DSL operand *is* an
`entity_reference` minus the top 16.

### `server_reference : u16` and its roles

```
u16 server_reference
  u8 realm_id                     bits 8–15    geographic; = realm_reference
  u8 server_id                    bits 0–7
```

`worker_reference` and `shard_reference` are **aliases of `server_reference`** — same 16 bits, same
layout. A worker and a shard *are* servers; the distinct name states which role a field means
(`event_log.worker_reference` is the subscription key). They are not wrapper types with an interior.

Realm is a **functional unit** — all servers in a realm work together. An `object_reference` carries
no realm, so objects are **not unique between realms**; `(server_reference, object_reference)` is
already realm-unique, so cross-realm identity costs zero extra bits.

`SERVER_REF_NONE = 0` — no server (never a real shard).

---

## Cold storage

### `cold_row_reference : u64` — the row key

A cold row holds many objects sharing `(type, subtype, region, zone, layer)`; those header fields
*are* its identity, so the key is their composite, not a surrogate.

```
u64 cold_row_reference
  u28 reserved                    bits 36–63
  u16 macro_position_reference    bits 20–35   region | zone (realm implied by the shard)
  u16 type_reference              bits 4–19    type_id | subtype_id
  u4  layer_id                    bits 0–3     ← must be in the key; see below
```

> **`layer_id` must be in the key.** `layer` is a tile-slot, not a type property, so it is *not*
> inside `type_reference`. Two rows sharing `(macro_position, type, subtype)` but differing in
> `layer` are distinct rows — omit `layer_id` and they collide.

### `kind_pos_reference : u32` — one per object in a cold row

The per-object delta. Everything else reconstructs from the row header.

```
u32 kind_pos_reference
  u16 kind_reference              bits 16–31   kind_id | variant_id
  u8  tile_reference              bits 8–15    the object's cell (x:4|y:4)
  u8  data                        bits 0–7     per-instance state
```

Each object costs **32 bits** carrying its full delta:

| you want | row gives | + entry gives |
|---|---|---|
| `definition_reference` | `type_reference` | `kind_reference` |
| `position_reference` | `macro_position` + `layer_id` + `type_id` | `tile_reference` |
| state | *(decode picked by `type_id`)* | `data` |

### `data : u8` — per-instance state

Rows are type-homogeneous, so the decode is read once per row. Currently **one universal decode**;
`type_id` selects it so it can diverge later without changing the layout.

```
u8 data
  u3 sub_position                 bits 5–7     8 offsets internal to the tile
  u2 rotation                     bits 3–4     4 facings (west mirrors east)
  u3 aux                          bits 0–2     type-decoded (e.g. count)
```

**`data:8` is the pack criterion, not a limit.** An object whose state doesn't fit stays hot, keeps
its full `hot_reference`, and carries a `state` row instead. Pawns never qualify → always hot. Dirt
tiles always qualify → cold. We never widen the per-tile cost to serve the stateful minority.

### `cold_removed` — the tombstone

Shares `cold_row_reference` 1:1 with its row, so a tombstone is just a **`tile_reference : u8`** —
macro_position, type and layer are already in the key and never repeated.

---

## `event_word : u64` — the DSL word

```
u64 event_word
  u4  op_code                     bits 60–63
  u12 reserved                    bits 48–59
  u16 server_reference            bits 32–47   ┐ low 48 = the qualified reference,
  u32 payload                     bits 0–31    ┘ plain-mask-identical to entity_reference
```

---

## `valid_at : u64` — the bitemporal row key

```
u64 valid_at
  u48 time_ms                     bits 16–63   dominates the ordering
  u16 sequence                    bits 0–15    tie-breaks writes within one ms
```

---

## Enumerations

**`reference_id : u6`** — which *reference variant* an `object_reference` is. Append-only. Names the
variant, **not** the game-type (that's `definition_reference.type_id`).

| name | value |
|---|---|
| `REF_NONE` | 0 |
| `REF_HOT` | 1 |
| `REF_COLD` | 2 |
| `REF_POSITION` | 3 |
| `REF_EVENT` | 4 |
| `REF_SERVER` | 5 |

**`type_id : u4`** — the game-type, in `type_reference`.

| name | value |
|---|---|
| `TYPE_NONE` | 0 |
| `TYPE_BIOME_TILE` | 1 |
| `TYPE_BIOME_THING` | 2 |
| `TYPE_PAWN` | 3 |
| `TYPE_PLAYER` | 4 |
| `TYPE_EVENT` | 5 |
| `TYPE_SERVER` | 6 |

**`op_code : u4`** — the DSL word's operation.

| name | value |
|---|---|
| `OP_LITERAL` | 0 |
| `OP_OBJECT` | 1 |
| `OP_ACTION` | 2 |
| `OP_ALIAS` | 3 |

---

## Legacy — retiring, do not build on

**`zone_id : u32`** — the world-global flat zone address, superseded by
`macro_position_reference` + `server_reference.realm_id`. Still live in `packed.rs` and the `index`
module's routing; retiring it is tracked in [`work/spacetime-rewrite/todo.md`](work/spacetime-rewrite/todo.md).

```
u32 zone_id                       ── LEGACY
  u8 realm                        bits 24–31
  u8 region                       bits 16–23
  u8 zone                         bits 8–15
  u8 reserved                     bits 0–7
```

`region_id` = `zone_id & REGION_ID_MASK` (0xFFFF_0000) — realm | region, the shard-routing key. The
within-realm `region | zone` slice **is** `macro_position_reference`, which is why the two coexist.

Also legacy in `packed.rs`, from the pre-0.2.3 model: `thing : u64`
(`kind:16 | x:4 | y:4 | data:5 | layer:3 | variant:5 | reserved:27`), `tile : u8`, `offset : u8`,
and the `THING_*_MAX` bounds. Superseded by `kind_pos_reference` + `data`.
