# Spatial references — position → zone → region → realm

> ⚠️ **Superseded + RETIRED in code (2026-07-14).** The authoritative object reference model is
> now [reference-model.md](../reference-model.md) — the honest **definition / position / data** split
> (all `u32`). The v1 layouts below (a `u64 object_reference` with `subkind` + packed `x/y/data`)
> **no longer match the code** — `object.rs`/`refs.rs` have been re-cut to reference-model.md and
> browser-verified. Kept here as historical rationale (and the `u8` spatial-nibble primitive, which
> survives). Read reference-model.md for the live shape.


The world is a **nested 16×16 grid**, four levels deep. Each level is one `u8`:
two `u4` coordinates, `hi:4 | lo:4`. The same primitive at four scales.

```
u8 spatial reference
bit  7        4 3        0
    ┌──────────┬──────────┐
    │  hi : 4  │  lo : 4  │      hi = x / *_x   (0..16)
    └──────────┴──────────┘      lo = y / *_y   (0..16)
```

Canonical impl: [`shared/codec/src/object.rs`](../../../../../../shared/codec/src/object.rs).
See [reference-vs-id.md](reference-vs-id.md) for the "reference" vs "id" vocabulary.

## The four levels

From the whole world down to a single tile — each step subdivides one cell of the level
above into its own 16×16 grid:

```
realm_reference     u8 = realm_x:4  | realm_y:4      16×16 realms in the world
  └ region_reference u8 = region_x:4 | region_y:4     16×16 regions in a realm
      └ zone_reference u8 = zone_x:4 | zone_y:4         16×16 zones in a region
          └ position_reference u8 = x:4 | y:4             16×16 tiles in a zone
```

| reference            | u8 fields               | one cell = | grid within |
|----------------------|-------------------------|------------|-------------|
| `realm_reference`    | `realm_x:4 \| realm_y:4`   | a realm    | the world   |
| `region_reference`   | `region_x:4 \| region_y:4` | a region   | a realm     |
| `zone_reference`     | `zone_x:4 \| zone_y:4`     | a zone     | a region    |
| `position_reference` | `x:4 \| y:4`               | a tile     | a zone      |

Stacked, the four `u8`s give a tile's full geographic address in **32 bits**:
`realm . region . zone . position`. Per axis that's `16^4 = 65 536` tiles, so the world
is a `65 536 × 65 536` tile plane (~4.29 billion tiles).

## One primitive, four names

All four are packed and read by the **same** functions — there is no
`pack_zone_reference` / `pack_realm_reference` today; they share the primitive:

- `pack_position_reference(x, y) -> u8` — packs any level's `(hi, lo)` ([object.rs:232](../../../../../../shared/codec/src/object.rs:232)).
- `ref_hi(r) -> u8` — the `x` / `zone_x` / `region_x` / `realm_x` nibble.
- `ref_lo(r) -> u8` — the `y` / `zone_y` / `region_y` / `realm_y` nibble.

The level is context, not a tag in the byte: a `u8` spatial reference is just two
nibbles; *which* grid it indexes is known from where it's stored. (If we later want
type-safety per level, the fix is thin newtype wrappers over the one packer, not four
bit-layouts.)

## Composite spatial references

- **`macro_position_reference : u16`** = `region_reference:8 | zone_reference:8` — a reference
  **composed of two references**, and the macro half of a `position_reference:u32` (its micro half
  is `micro_position_reference:16` = `tile_reference:8 | layer_reference:8`). The zone-subscription
  key: a client names region + zone directly, no filter. It is also the macro half of the **cold row
  key** (`cold_row_reference:u64` = `macro_position:16 | type_reference:16 | layer_id:4`).

- **`cold_reference : u32`** stacks `region:8 | zone:8 | position:8 | layer_id:4 |
  type_id:4` — three spatial levels plus layer/type. Note it **omits `realm`**: a cold
  object lives in a realm-scoped shard, so the realm is carried by the server
  (`cold_server_id`), not repeated in every reference. See
  [hot-cold-references.md](hot-cold-references.md).

## Relationship to `zone_id` — RECONCILED (2026-07-14)

There are **two** ways a zone is named, at different scales (see the contrast in
[reference-vs-id.md](reference-vs-id.md)):

- **`zone_reference : u8`** (object model) — a zone's `(zone_x, zone_y)` *within its
  region*. Geographic, nestable, realm-relative.
- **`zone_id : u32`** (`packed.rs`) — the zone's **world-global address**, the routing /
  subscription key (`WHERE zone_id`).

**These are now the same geometry.** `zone_id` was repartitioned to
`realm_reference:8 | region_reference:8 | zone_reference:8 | reserved:8` — it *nests* the
references above. The old-game layout (`region_x:8 | region_y:8 | surface:8 | zone_x:4 | zone_y:4`,
with a **`surface`** z-axis) is **retired**; there is no flat/legacy zone id left to reconcile.
`region_of(zone_id)` masks to `realm | region` (the shard-routing key) and `zone_region_zone(zone_id)`
yields the `macro_position_reference:u16` (the within-realm cold/subscription key).
