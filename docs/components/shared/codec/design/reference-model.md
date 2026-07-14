# Design — the reference model (definition / position / data)

_The shape. Authoritative as of 2026-07-14; supersedes the v1 layout in
[`object-model.md`](object-model.md) §3 and the v1 parts of [`references/`](references/) (which
still describe the current **code** — the re-cut is a [`plan/`](../plan/) item). Rationale in
[`../intent/reference-model.md`](../intent/reference-model.md)._

An object is described by **three orthogonal references** — *what* it is, *where* it is, and its
*state* — each independently addressable. This is the honest split (v1 packed all three into one
`u64 object_reference`, which conflated a shared blueprint with per-instance position + state).

## `definition_reference : u32` — *what it is* (shareable, position-free)

```
 31     28 27              16 15              4 3          0
┌─────────┬──────────────────┬─────────────────┬───────────┐
│type_id:4│   subtype_id:12  │    kind_id:12   │ variant:4 │
├─────────┴──────────────────┼─────────────────┴───────────┤
│     type_reference : u16   │     kind_reference : u16     │
└────────────────────────────┴─────────────────────────────┘
```
| field | bits | width | range |
|---|---|---|---|
| `type_id` | 28–31 | 4 | 0–15 (0 = reserved null) |
| `subtype_id` | 16–27 | 12 | 0–4095 |
| `kind_id` | 4–15 | 12 | 0–4095 |
| `variant_id` | 0–3 | 4 | 0–15 |

`type_reference : u16` = `type_id | subtype_id`; `kind_reference : u16` = `kind_id | variant_id`.
Fully shareable — every instance of `pawn/human/male.fat` has the **same** `definition_reference`.
Capacity: **16 types × 4096 subtypes × 4096 kinds × 16 variants**. No `subkind` (dropped — a
combined kind like `male.fat` is one `kind_id`). No reserved headroom — the u32 is exact.

## `position_reference : u32` — *where it is* (the standalone cold address / DSL target)

```
 31            24 23           16 15           8 7    4 3      0
┌────────────────┬───────────────┬──────────────┬──────┬───────┐
│ region_ref:8   │  zone_ref:8   │  tile_ref:8  │type:4│layer:4│
├────────────────┴───────────────┼──────────────┴──────┴───────┤
│ macro_position_reference : u16 │  micro_position_reference:16 │
└────────────────────────────────┴──────────────────────────────┘
                                                └ layer_reference:8 ┘
```
| field | bits | width | meaning |
|---|---|---|---|
| `region_reference` | 24–31 | 8 | region within realm (`region_x:4 \| region_y:4`) |
| `zone_reference` | 16–23 | 8 | zone within region |
| `tile_reference` | 8–15 | 8 | tile within zone (`x:4 \| y:4`) |
| `layer_reference` | 0–7 | 8 | `type_id:4 \| layer_id:4` — the tile-slot, interpreted per type |

This is the form a DSL `OBJECT` operand hands over to **target** a cold object: it self-identifies
(carries `type_id` + `layer_id`), because there's no row alongside it. Uniqueness rule: **one
object per `(type, layer, tile)`** within a zone. (`realm` is not here — it's part of
`server_reference` / routing.)

## Cold row — factors the shared bits out, `data:8` per object

A cold row stores many objects that share `(type, subtype, region, zone, layer)`; only the
per-object delta is repeated.

**Row header (shared):**
| field | width | meaning |
|---|---|---|
| `type_reference` | u16 | `type_id \| subtype_id` |
| `macro_position_reference` | u16 | `region_ref \| zone_ref` |
| `layer_id` | u4 | the tile-slot (one row per layer) |

**`Vec<kind_pos_reference : u32>` — one per object:**
```
 31              16 15         8 7          0
┌──────────────────┬────────────┬────────────┐
│ kind_reference:16│tile_ref:8  │   data:8   │
└──────────────────┴────────────┴────────────┘
```
Each object costs **32 bits** carrying its full delta. Everything reconstructs from row + entry:

| you want | = row gives | + entry gives |
|---|---|---|
| `definition_reference` | `type_reference` | `kind_reference` |
| `position_reference` | `macro` + `layer_id` + `type_id` | `tile_reference` |
| state | *(decode picked by `type_id`)* | `data` |

## `data : u8` — per-instance state, decoded by the row's `type_id`

Because a row is **type-homogeneous**, the decode is read once per row and applied to every entry.
For now this may be **one universal decode** for all types; `type_id` selects it so it can diverge
later without changing the layout. Suggested default (coarse, adjustable per type):

| field | bits | width | meaning |
|---|---|---|---|
| `sub_position` | 5–7 | 3 | 8 offsets internal to the tile |
| `rotation` | 3–4 | 2 | 4 facings (W mirrors E) |
| `aux` | 0–2 | 3 | type-decoded (e.g. `count`) |

## The cold/hot boundary — `data:8` *is* "packable"

`data:8` is not a limit, it's the **pack criterion**. Cold exists to compress the mass common case
onto the wire cheaply; an object whose state doesn't fit in `data:8` **stays hot** — it keeps its
full id (`hot_reference` / `entity_reference`) and a `state` row carrying its richer state, and
transmits a bit more per object. The pack action gates on this:

- **GC / worker never queue a pack** for an object too state-dense for cold; and if a pack is
  attempted, it **skips/fails** objects that don't fit.
- **Pawns** (inventories, needs, …) never qualify → always hot. **Dirt tiles** always qualify →
  cold, compressed a-bazillion-fold.

We never widen the per-tile cost to serve the stateful minority — they're hot by definition.

## What changed from v1
- `u64 object_reference` (type+kind+position+data) → **`u32 definition_reference`** (type+kind
  only). Frees the name `object_reference` for an object *handle*.
- Dropped `subkind` (was `u4`); `kind_id` widened `u10 → u12`.
- `x/y` (position) and `data` moved **out** of the kind half → position lives in
  `position_reference` / the cold `tile_reference`; state lives in `data:8` (cold) or the `state`
  table (hot).
- `layer` moved from the type half to `position_reference` (it's a tile-slot, not a type property).
- `region_zone` (macro_position) is now explicit (`region_ref:8 | zone_ref:8`) — closes
  divergence #4.
