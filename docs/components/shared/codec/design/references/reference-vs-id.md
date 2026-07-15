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

| id             | width | allocated by | namespace |
|----------------|-------|--------------|-----------|
| `type_id`      | 4     | code palette | structural, append-only (`TYPE_*`) |
| `subtype_id`   | 12    | registry     | content-derived (biome, species, …) |
| `kind_id`      | 12    | registry     | content-derived |
| `variant_id`   | 4     | registry     | content-derived |
| `layer_id`     | 4     | code palette | tile object-slot |
| `hot_reference`| 32    | per-server monotonic counter | that server's mint sequence (a leaf despite the name — see below) |
| `realm_id`     | 8     | geographic   | `realm_x:4 \| realm_y:4` within the world |
| `server_id`    | 8     | per-realm    | id *within* a `realm_id` |
| `event_reference` | 32 | per-shard auto_inc | that shard's `event_log` PK (a leaf despite the name) |

`subkind_id` was **dropped** (2026-07-14 re-cut — flattened into `kind_id`, widened 10→12).
`action_id`/`data_type` are **gone** with the functional `action_reference` (the DSL word carries an
`ACTION_*` verb id in its payload instead). `count` and the `x`/`y` grid coordinates are leaf
scalars too, but they're magnitudes/coordinates rather than allocated identities, so they don't take
the `_id` suffix.

> ⚠️ **Two leaves carry a `_reference` name** — `hot_reference` and `event_reference` are plain
> allocator counters (ids by this doc's rule), but they're named `_reference` because they occupy an
> `object_reference` **variant slot** (the union is addressed uniformly). The name marks the slot,
> not the structure. Known, deliberate exception.

## references (the packed composites)

Each packs the ids/references named, in fixed bit positions:

| reference                | width | composed of |
|--------------------------|-------|-------------|
| `definition_reference`   | 32    | `type_reference` + `kind_reference` (= `type_id`+`subtype_id`+`kind_id`+`variant_id`) |
| `type_reference`         | 16    | `type_id` + `subtype_id` (the shareable type half) |
| `kind_reference`         | 16    | `kind_id` + `variant_id` (the kind half) |
| `object_reference`       | 32    | *a tagged union* — one of `hot_reference` / `cold_reference` / `position_reference` / `event_reference` / server; variant named by `reference_id` |
| `entity_reference`       | 64    | `reserved:10` + `reference_id:6` + `server_reference` + `object_reference` |
| `server_reference`       | 16    | `realm_id` + `server_id` (geographic) |
| `position_reference`     | 8     | `x` + `y` — the u8 nibble primitive (also `zone_`/`region_`/`realm_reference`) |
| `zone_reference`         | 8     | `zone_x` + `zone_y` |
| `region_reference`       | 8     | `region_x` + `region_y` |
| `realm_reference`        | 8     | `realm_x` + `realm_y` |
| `macro_position_reference` | 16  | `region_reference` + `zone_reference` (a reference of references) — the macro half of `position_reference` |
| `micro_position_reference` | 16  | `tile_reference` + `layer_reference` — its micro half |
| `cold_row_reference`     | 64    | `macro_position_reference` + `type_reference` + `layer_id` — the **cold row's** identity (distinct from `cold_reference` below, which addresses an **object**) |
| `layer_reference`        | 8     | `type_id` + `layer_id` |
| `cold_reference`         | 32    | `region_reference` + `zone_reference` + tile + `layer_reference` (realm rides `server_reference`) |
| `zone_id`                | 32    | `realm_reference` + `region_reference` + `zone_reference` (+ reserved) — see the contrast below |

References may nest: `macro_position_reference` packs two `*_reference`s; `definition_reference` packs
two 16-bit references, each of which packs ids. **Retired (2026-07-14):** the v1 `object_reference:64`
(type+kind halves), `object_type_reference` / `object_kind_reference`, `action_reference`, and the
`zone_reference:64` whole-zone aggregate — all deleted from the codec.

## The naming convention in code

- Constructors pack: `pack_<thing>_reference(...) -> uN`.
- Accessors unpack a field back out: `<thing>_ref_<field>(r) -> ...`, returning an id (or
  a nested reference). e.g. `type_ref_type_id`, `kind_ref_x`, `cold_ref_macro_position`.
- A `pack_*` never takes another `pack_*`'s output *as an id* — it takes it as a
  reference. The signature type is the tell.

## A worked contrast: `zone_id` vs `zone_reference`

- **`zone_reference : u8`** — `zone_x:4 | zone_y:4`, a zone's coordinate *within its region*. A
  composite of two coordinate ids. Used to build `macro_position_reference`.
- **`zone_id : u32`** — the **world-global address** of one zone, used as the routing/subscription
  key (`WHERE zone_id`). Since the 2026-07-14 geometry re-cut it is
  `realm_reference:8 | region_reference:8 | zone_reference:8 | reserved:8` — i.e. it now *nests*
  the three geographic references (it was a flat, allocated, world-global number, and the old-game
  `surface` byte is gone).

> ⚠️ **Naming tension (known, deliberate).** Post-re-cut `zone_id` is a **packed composite**, so by
> the rule above it is really a *reference*, not an id — you can bit-shift `realm`/`region`/`zone`
> out of it. It keeps the `_id` name because it is the **routing key** the whole edge/index/client
> subscription path addresses zones by (`SubZone`, `WHERE zone_id`), and `zone_reference:u8` already
> owns the within-region meaning. Renaming it (e.g. `world_zone_reference`) is a rename across
> edge + index + protocol + client — deliberately not done; flagged here so the exception is
> explicit rather than an accident.

When you read `<thing>_id` vs `<thing>_reference` in this codebase, the category is what it's
telling you — with `zone_id` (and the `hot_reference`/`event_reference` leaves above) as the
documented exceptions.
