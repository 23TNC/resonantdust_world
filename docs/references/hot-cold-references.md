# Hot and cold references

Every object is in one of two states, and each state has its own `u32` reference:

- **cold** — settled, packed, static. Addressed **geographically** by where it sits:
  `cold_reference`. Stored in the cold table as a `Vec` under a shared type half.
- **hot** — minted, actively ticking through the `event → state_log → state` pipeline.
  Addressed by a minted id: `hot_reference` (= `object_id`).

Both are `u32`, on purpose: an action slot / target list holds "a reference" in 32 bits,
and the action's slot definition says whether that slot is a `cold_reference` or a
`hot_reference` — no discriminator bit. To *touch* a cold object you must make it hot
first (`unpack`/`mint_hot`), so the bridge below is how objects cross between the two.
See [`docs/object-model.md`](../object-model.md) §"cold-vs-hot" and
[reference-vs-id.md](reference-vs-id.md) for the vocabulary.

---

## `cold_reference : u32` — implemented

`pack_cold_reference(...)` — [object.rs:280](../../shared/codec/src/object.rs:280).

```
bit  31            24 23           16 15               8 7        4 3        0
    ┌────────────────┬───────────────┬───────────────────┬──────────┬──────────┐
    │ region_ref : 8 │  zone_ref : 8 │  position_ref : 8 │ layer:4  │ type_id:4│
    └────────────────┴───────────────┴───────────────────┴──────────┴──────────┘
```

| field                | bits  | width | meaning |
|----------------------|-------|-------|---------|
| `region_reference`   | 24–31 | 8     | region within the realm (`region_x:4 \| region_y:4`) |
| `zone_reference`     | 16–23 | 8     | zone within the region |
| `position_reference` | 8–15  | 8     | tile within the zone (`x:4 \| y:4`) |
| `layer_id`           | 4–7   | 4     | tile object-slot (floor / wall / affixed / …) |
| `type_id`            | 0–3   | 4     | structural type |

**Subtype-agnostic by design.** The uniqueness rule is *one object per
`(type, layer, tile)`*, so the cold reference needs only `type_id`, not `subtype_id` — a
cell can't hold two objects of the same type+layer. (The full `subtype/kind/variant` live
in the object's `object_kind_reference` inside the cold row, not in its address.)

**Realm is implied, not encoded.** A cold object lives in a **realm-scoped shard**, so
the realm is named once by the shard, not repeated per object. Pair a `cold_reference`
with a **`cold_server_id : u8`** to name the shard within the realm
([object.rs:269](../../shared/codec/src/object.rs:269)).

Its subscription key is `cold_ref_region_zone(r) -> u16` — the `region_zone_reference`,
dropping position/layer/type ([object.rs:321](../../shared/codec/src/object.rs:321)).

---

## `hot_reference : u32` — **specified, not yet implemented**

The intended cold→hot counterpart:

```
bit  31                                                                        0
    ┌──────────────────────────────────────────────────────────────────────────┐
    │                          object_id : 32                                    │
    └──────────────────────────────────────────────────────────────────────────┘
```

| field       | bits | width | meaning |
|-------------|------|-------|---------|
| `object_id` | 0–31 | 32    | the minted id of a hot (active) object |

`unpack` / `mint_hot` **consumes a `cold_reference` and produces a new `hot_reference`**
(mints the `object_id`); `pack` folds a settled hot object back to a `cold_reference` at
its resting tile. A queued action pinned to a cold object *symbolically* is rebound
`cold_reference → hot_reference` when `mint_hot` runs (`object-model.md` §rebinding).

### Implementation status — this is a gap

There is **no `hot_reference` type in code today** (`rg hot_reference` → nothing). The
32-bit `object_id` currently exists only as a **slice of the `u64` `entity_reference`**
([`refs.rs`](../../shared/codec/src/refs.rs)):

```
entity_reference : u64 = entity_type:8 | entity_id:32 | mint_server:16 | reserved:8
                                         └─────────┬─────────┘
                                    the client extracts this as "object_id"
```

The client pulls `object_id` out for JS safety (*"32-bit `object_id` (the minted
`entity_id`, JS-safe)"*, [api.rs:149](../../client/core/src/api.rs:149);
[web.rs:584](../../client/core/src/web.rs:584),
[engine.rs:519](../../client/core/src/engine.rs:519)). So the *value* exists; the *named
`u32` abstraction* and the `pack_hot_reference` / `cold_reference ⇄ hot_reference` bridge
do not.

**To close it:** define `hot_reference` in `object.rs` beside `cold_reference`, and
reconcile it against the `u64` `entity_reference` — decide whether a hot object's identity
is the compact `u32 object_id` (with `entity_type` / `mint_server` carried elsewhere) or
stays the full `u64`. That reconciliation is open.

---

## The bridge, at a glance

```
        unpack / mint_hot  (mints object_id)
   cold ────────────────────────────────────▶ hot
   cold_reference:u32                          hot_reference:u32 (= object_id)
   (region.zone.position.layer.type)           (minted)
        ◀────────────────────────────────────
                 pack  (settles at a tile)
```

- **cold** is addressed by *place*; **hot** by *mint*.
- Only hot objects tick. Cold objects are inert until unpacked.
- An action targets either; you `mint_hot` a cold target before acting on it.
