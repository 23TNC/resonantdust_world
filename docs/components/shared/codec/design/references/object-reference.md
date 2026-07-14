# `object_reference`

> ⚠️ **Reference layouts superseded (2026-07-14).** The authoritative object reference
> model is now [reference-model.md](../reference-model.md) — the honest **definition / position / data**
> split (all `u32`). The layouts below still describe the current **code** (v1: a `u64
> object_reference` with `subkind` and `x/y/data` packed into the kind half); the re-cut is a
> [plan](../../plan/) item. Read reference-model.md for the target shape.


The packed identity of a single **object** — the 0.2.3 object-model unit that makes
tiles and things the same kind of thing. One `u64`, split into two `u32` halves: a
**shared** type half and a **per-instance** kind half.

Canonical implementation: [`shared/codec/src/object.rs`](../../../../../../shared/codec/src/object.rs)
(pure integer math + roundtrip tests). This document is the spec that file implements;
if they disagree, the code wins and this doc is the bug.

See [reference-vs-id.md](reference-vs-id.md) for what "reference" and "id" mean as
words — this doc assumes that vocabulary.

```
object_reference : u64
┌───────────────────────────────┬───────────────────────────────┐
│  object_type_reference : u32  │  object_kind_reference : u32  │
│         (high 32)             │          (low 32)             │
└───────────────────────────────┴───────────────────────────────┘
        the SHARED half                  the PER-INSTANCE half
```

- **type half** — what *sort* of object this is: `(type, subtype, layer)`. Many objects
  share one type half. The cold store exploits this: a cold row is **one type half + a
  `Vec` of kind halves**, so the shared bits are stored once per row.
- **kind half** — this *particular* object: its kind/subkind/variant, its tile within
  the zone (`x`/`y`), and a small type-decoded `data` payload.

`pack_object_reference(type_reference, kind_reference)` composes the two;
`object_ref_type_reference` / `object_ref_kind_reference` split them back out.

---

## `object_type_reference : u32` — the shared half

```
bit  31            28 27                    16 15        12 11                 0
    ┌────────────────┬────────────────────────┬────────────┬───────────────────┐
    │   type_id : 4  │     subtype_id : 12    │  layer : 4 │   reserved : 12   │
    └────────────────┴────────────────────────┴────────────┴───────────────────┘
```

| field        | bits  | width | range      | meaning |
|--------------|-------|-------|------------|---------|
| `type_id`    | 28–31 | 4     | `1..=15`   | the object's structural type. Implies a pipeline + a `data` decode, so it lives in code, not content. `0` is the null/unset sentinel. |
| `subtype_id` | 16–27 | 12    | `0..=4095` | content-derived sub-classification. For `biome-tile`/`biome-thing` the subtype **is the biome**; for a pawn it is the species. |
| `layer`      | 12–15 | 4     | `0..=15`   | the tile object-slot the object occupies (floor / wall / affixed / …). NOT a texture `part`. |
| `reserved`   | 0–11  | 12    | —          | growth room; stays `0`. |

`pack_type_reference(type_id, subtype_id, layer)` /
`type_ref_type_id` · `type_ref_subtype_id` · `type_ref_layer`.

### `type_id` palette (structural, append-only)

Assigned in code, not content — types are few and fixed; kinds are content-derived.
Append a new type at the end so stored zones never renumber.

| id | name              | notes |
|----|-------------------|-------|
| 0  | `TYPE_NONE`       | null/unset sentinel |
| 1  | `TYPE_BIOME_TILE` | biome-classified ground; `subtype = biome` |
| 2  | `TYPE_BIOME_THING`| biome scatter (flora, rocks); `subtype = biome` |
| 3  | `TYPE_PAWN`       | mobile agent; `subtype = species` |
| 4  | `TYPE_PLAYER`     | player-controlled entity |
| 5  | `TYPE_EVENT`      | an event-log entry (the generalized object log) |
| 6  | `TYPE_SERVER`     | a server / shard (provenance) |

---

## `object_kind_reference : u32` — the per-instance half

```
bit  31          22 21      18 17        14 13     10 9      6 5            0
    ┌──────────────┬──────────┬────────────┬─────────┬────────┬────────────┐
    │ kind_id : 10 │ subkind:4│ variant : 4│  x : 4  │  y : 4 │  data : 6  │
    └──────────────┴──────────┴────────────┴─────────┴────────┴────────────┘
```

| field        | bits  | width | range      | meaning |
|--------------|-------|-------|------------|---------|
| `kind_id`    | 22–31 | 10    | `0..=1023` | content-derived kind within the (type, subtype). |
| `subkind_id` | 18–21 | 4     | `0..=15`   | finer classification under the kind. |
| `variant_id` | 14–17 | 4     | `0..=15`   | interchangeable visual/stat variant of the kind. |
| `x`          | 10–13 | 4     | `0..=15`   | tile column within the zone (a `u4` grid coord). |
| `y`          | 6–9   | 4     | `0..=15`   | tile row within the zone. |
| `data`       | 0–5   | 6     | `0..=63`   | **type-decoded** payload — see below. |

`pack_kind_reference(kind_id, subkind_id, variant_id, x, y, data)` /
`kind_ref_kind_id` · `kind_ref_subkind_id` · `kind_ref_variant_id` · `kind_ref_x` ·
`kind_ref_y` · `kind_ref_data`.

`x`/`y` together are exactly a `position_reference` (`x:4 | y:4`); `kind_ref_position`
returns it as the packed `u8`.

### The `data : 6` payload (type-decoded)

The low 6 bits are decoded as a function of `type_id` — the caller that knows the type
picks the decoder. Two decoders exist today:

```
default (most types)          stackable (dedicated resource type)
┌──────────┬────────────┐     ┌────────────────────────┐
│ rot : 2  │  count : 4 │     │      count : 6         │
└──────────┴────────────┘     └────────────────────────┘
 rotation 0..4 (facings;       count 0..64 — the whole
 west = mirrored east)         6 bits, no rotation
 count 0..16
```

- `data_default(data) -> (rotation, count)` / `pack_data_default(rotation, count)`
- `data_stack(data) -> count` / `pack_data_stack(count)`

The **bit positions never change** — only the *interpretation* of those 6 bits does.
For a normal object this is `rotation:2 | count:4`; a stackable/resource type that never
rotates spends all 6 bits on `count` to reach 64.

---

## Related references (context)

These are separate references, not part of `object_reference`, but an object is addressed
and stored through them — each documented in its own file:

- **Spatial ladder** — `position → zone → region → realm`, each a `u8 = hi:4 | lo:4`, all
  packed by the one `pack_position_reference` primitive (there are no per-level
  constructors). The kind half's `x`/`y` *is* a `position_reference`. See
  [spatial-references.md](spatial-references.md).
- **Hot / cold** — a settled object is addressed geographically by a `cold_reference:u32`;
  an active one by a minted `hot_reference:u32` (`= object_id`). `unpack`/`pack` bridge
  the two. See [hot-cold-references.md](hot-cold-references.md).

---

## Why two halves

Splitting `(type, subtype, layer)` from `(kind, subkind, variant, x, y, data)` is what
lets the cold store fold a zone into a handful of rows: one row per
`(zone, type_reference)` holding a `Vec<object_kind_reference>`. The shared 32 bits are
written once; each settled object costs only its 32-bit kind half. `unpack` promotes one
kind half to a hot `state` entity; `pack` folds a settled hot entity back into the Vec.
See [`docs/object-model.md`](../object-model.md) and
[`docs/data-shards.md`](../../../../../archive/data-shards.md).
