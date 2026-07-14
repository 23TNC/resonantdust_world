# "reference" vs "id"

Two words with two precise, non-overlapping jobs across the codec
([`shared/codec/`](../../../../../../shared/codec/src)) and the shard/pipeline. Keeping them
straight is the whole reason the packed layouts read cleanly, so this is a naming
contract, not a style preference.

## The rule

- **id** — a **leaf scalar**. A plain integer with no internal structure, drawn from an
  allocator: a registry counter, a monotonic mint counter, or a fixed
  namespace/coordinate. An id is an *ingredient*. Named `<thing>_id`.

- **reference** — a **packed composite**. A fixed-width integer built by bit-packing one
  or more ids (and/or smaller references) into a single addressable, storable,
  wire-safe identity. A reference is the *dish*. Named `<thing>_reference`.

> A reference is composed of ids (and smaller references). An id is never composed of
> references. If you can meaningfully bit-shift fields *out* of it, it's a reference; if
> it's just a number you were handed by an allocator, it's an id.

## ids (the leaves)

Each comes from exactly one allocator and carries no sub-structure:

| id            | width | allocated by | namespace |
|---------------|-------|--------------|-----------|
| `type_id`     | 4     | code palette | structural, append-only (`TYPE_*`) |
| `subtype_id`  | 12    | registry     | content-derived (biome, species, …) |
| `kind_id`     | 10    | registry     | content-derived |
| `subkind_id`  | 4     | registry     | content-derived |
| `variant_id`  | 4     | registry     | content-derived |
| `layer_id`    | 4     | code palette | tile object-slot |
| `zone_id`     | 32    | world-global | flat, allocated per zone |
| `region_id`   | —     | world-global | flat |
| `entity_id`   | 32    | per-server monotonic counter | that server's mint sequence |
| `server_id`   | 10    | per-type range | id *within* a `server_type` |
| `action_id`   | 10    | code/registry | id within a `data_type` |

`count` and the `x`/`y` grid coordinates are leaf scalars too, but they're
magnitudes/coordinates rather than allocated identities, so they don't take the `_id`
suffix.

## references (the packed composites)

Each packs the ids/references named, in fixed bit positions:

| reference                | width | composed of |
|--------------------------|-------|-------------|
| `object_reference`       | 64    | `object_type_reference` + `object_kind_reference` |
| `object_type_reference`  | 32    | `type_id` + `subtype_id` + `layer` (+ reserved) |
| `object_kind_reference`  | 32    | `kind_id` + `subkind_id` + `variant_id` + `x` + `y` + `data` |
| `entity_reference`       | 64    | `entity_type` + (`entity_id` + `mint_server`) *or* (`zone_id` + location + layer) |
| `server_reference`       | 16    | `server_type` + `server_id` |
| `action_reference`       | 16    | `data_type` + `action_id` |
| `position_reference`     | 8     | `x` + `y` |
| `zone_reference`         | 8     | `zone_x` + `zone_y` |
| `region_reference`       | 8     | `region_x` + `region_y` |
| `realm_reference`        | 8     | `realm_x` + `realm_y` |
| `region_zone_reference`  | 16    | `region_reference` + `zone_reference` (a reference of references) |
| `cold_reference`         | 32    | region + zone + position + `layer_id` + `type_id` |
| `zone_reference` (agg.)  | 64    | `server_id` + `zone_id` — the whole-zone aggregate key in `refs.rs` |

References may nest: `region_zone_reference` packs two `*_reference`s;
`object_reference` packs two 32-bit references, each of which packs ids.

## The naming convention in code

- Constructors pack: `pack_<thing>_reference(...) -> uN`.
- Accessors unpack a field back out: `<thing>_ref_<field>(r) -> ...`, returning an id (or
  a nested reference). e.g. `type_ref_type_id`, `kind_ref_x`, `cold_ref_region_zone`.
- A `pack_*` never takes another `pack_*`'s output *as an id* — it takes it as a
  reference. The signature type is the tell.

## A worked contrast: `zone_id` vs `zone_reference`

They are deliberately different things, and the names say so:

- **`zone_id : u32`** — a flat, world-global allocated number naming one zone. A leaf.
  Used as a routing key (`WHERE zone_id`) and inside the 64-bit aggregate
  `zone_reference`.
- **`zone_reference : u8`** — `zone_x:4 | zone_y:4`, a zone's coordinate *within its
  region*. A composite of two coordinate ids. Used to build `region_zone_reference`.

Same stem, opposite category — because one is an allocated identity and the other is a
packed coordinate address. When you read either word in this codebase, that category is
what it's telling you.
