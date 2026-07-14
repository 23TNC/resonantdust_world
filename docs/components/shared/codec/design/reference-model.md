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

## `position_reference : u32` — *where it is* (the positional layout)

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

This is the positional layout: it self-identifies an object-slot (`type_id` + `layer_id`),
because there's no row alongside it. Uniqueness rule: **one object per `(type, layer, tile)`**
within a zone. (`realm` is not here — it's part of `server_reference` / routing.)

**`position_reference` and `cold_reference` are two reference *types* that share this layout —
not one thing.** A `position_reference` is *a location*: **any** object has one (a hot pawn's
tile is a `position_reference`), and events that need a tile carry it. A `cold_reference` is *a
cold object addressed by its position* — the same 32 bits, but it denotes "the settled object
there, `unpack` it hot." A hot object never has a `cold_reference`; a cold object's position **is**
one. You can `unpack` a `cold_reference` (or a `position_reference` that lands on a cold object —
which makes it one); you can never `unpack` a `hot_reference`. They're distinguished by
`reference_id` (below) or by context — never by their bits.

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

## Identity — the handle side (`object_reference` / `entity_reference`)

`definition_reference` says *what*; these say *which*. Everything in the game is an object, so one
u32 addresses any of them.

**`object_reference : u32`** — the universal handle, a tagged union whose variants share the width
but not the meaning; disambiguated by **context** (the container/action slot) or, when carried
standalone, by the `reference_id` in an `entity_reference`:

| variant | is | notes |
|---|---|---|
| `hot_reference` | a live/minted object | per-server id; can't be `unpack`ed (already hot) |
| `cold_reference` | a settled object at a position | `unpack`able → hot; positional layout |
| `position_reference` | a location | any object has one; a bare tile for an event |
| `event_reference` | an event row | what `ALIAS`/`AWAIT` carry |
| server (as `reserved:16 \| server_reference:16`) | a server, as an object | |

`cold_reference` and `position_reference` share the positional layout above but are **distinct
types** — see that section.

**`entity_reference : u64`** — an `object_reference` made globally unique and (optionally)
self-describing:
```
 63          54 53      48 47              32 31                    0
┌──────────────┬──────────┬──────────────────┬──────────────────────┐
│ reserved:10  │ ref_id:6 │ server_reference │  object_reference:32 │
└──────────────┴──────────┴──────────────────┴──────────────────────┘
              (which variant)  └──── low 48 = the DSL word's qualified reference ────┘
```
| field | bits | width | meaning |
|---|---|---|---|
| `reserved` | 54–63 | 10 | headroom |
| `reference_id` | 48–53 | 6 | which reference type the `object_reference` is (64 types; append-only, 0 = none) |
| `server_reference` | 32–47 | 16 | the server the object lives on / is qualified by |
| `object_reference` | 0–31 | 32 | the handle |

The **low 48 bits (`server_reference:16 | object_reference:32`) are identical to the DSL word's
qualified reference** ([`shard/design/event-dsl.md`](../../../server/spacetime/modules/shard/design/event-dsl.md) —
`op_code:4 | reserved:12 | server_reference:16 | payload:32`), plain-mask extractable from either.
So a DSL operand **is** an `entity_reference` minus the top 16 (op-tag vs `reserved|ref_id`). Bare
`object_reference` relies on context; carry the full `entity_reference` when you must be
unambiguous (`reference_id` names the variant).

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
- **`entity_reference` re-laid-out** to `reserved:10 | reference_id:6 | server_reference:16 |
  object_reference:32` (was `entity_type:8 | entity_id:32 | mint_server:16`): the type left for
  `definition_reference`, `entity_id` generalized to the `object_reference` union, and the low 48
  now matches the DSL word. This settles the `hot_reference` re-key (divergence #5) — a hot object
  is `object_reference = hot_reference:32`, server-qualified.
