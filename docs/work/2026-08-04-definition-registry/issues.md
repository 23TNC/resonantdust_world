# Issues — definition registry

_Measured on 2026-08-04, before planning, against the live tree._

## I1 — three hot paths unpack the def id's bits; one is a safety property {#i1}

This is what makes [F3](forks.md#f3) forced rather than chosen. Every site reads the packed fields
with no registry available:

| Site | Reads | Why it can't take a lookup |
|---|---|---|
| [`worker/main.rs:495`](../../../server/worker/src/main.rs) | `def_type_id` | routes `CREATE` to a type's shard arm and **rejects an unknown type BY NAME rather than silently pawning it** — inside the sim, per event |
| [`worker/main.rs:92`](../../../server/worker/src/main.rs) | `def_type_id` | `tics_for` branches on `TYPE_PAWN` to decide how to key the speed table |
| [`wasm/lib.rs:523-532`](../../../shared/wasm/src/lib.rs) | `def_kind_id`, `def_type_id`, `def_variant_id` | the per-cell render decode: namespace, visual-table index, subframe select — per cell, per zone |

Note what is *absent*: nothing reads `def_kind_id` and expects a stable meaning across versions. The
render decode uses it as an opaque table index, which is precisely why [F5](forks.md#f5) can burn
kind ids for versioning without touching a consumer.

## I2 — `def_subtype_id` has ZERO runtime readers — the def's subtype half is write-only {#i2}

`def_subtype_id` exists in the codec, the npc packs species into it via `pack_definition_from_ids`,
and **nothing ever reads it back out**. Subtype is read from the cold row header instead
(`cold_row_subtype`, a different field on a different record).

So 12 bits of every `definition_reference` are currently written and never decoded. That is either
free space to spend on [I3](#i3)'s ceiling, or a sign the def should not carry subtype at all and
should read it from the row. **It must be settled before the registry pins the layout** — see
[B2](blockers.md#b2) — because that is the moment it becomes expensive to change.

## I3 — `variant_id` is u4 (16 slots) and the wolf already holds 15 {#i3}

From the live art manifest:

```
pawn/animal/wolf var_variant:
  [1, 124, 125, 555, 777, 888, 7001, 7002, 8101, 8102, 8201, 8202, 8301, 8302, 1682748910]   → 15
```

Those are the art tree's variant **labels**, not slots — one is ten digits. The registry handles that
(`string variant` in the row, u4 slot in the id), so labels are free. What is not free is the
**count**: `VARIANTS_PER_DEF = 16` and one kind is at 15 of 16 **today**. This is a live ceiling, not
a future one, and it interacts directly with [I2](#i2) — dropping the write-only subtype half would
free 12 bits, some of which could widen variant. See [B2](blockers.md#b2).

## I4 — the taxonomy is not uniformly three segments {#i4}

Every `texture` value in the corpus today:

```
biome-thing/default/conifer     biome-thing/default/flora
pawn/animal/wolf                pawn/human/female            pawn/human/male
biome-tile/default/smooth/wall  ← FOUR segments
white                           ← ONE segment, a sentinel
```

`white` is not a path — it is "no art, flat tint", and defs using it (shrub, cactus, the torches, the
wall blueprint) have no texture tree at all. `biome-tile/default/smooth/wall` has a fourth segment
whose role is ambiguous: `smooth` could be the kind with `wall` a named form/variant, or `default/
smooth` could be a two-level subType.

The taxonomy fields cannot be authoritative until both are resolved — [B1](blockers.md#b1).

## I5 — the registry must be GLOBAL, but shards are per-region {#i5}

`definition_reference`s appear in every shard's stored rows, so the registry is world-global by
definition. The existing per-env global DB is `index` (routing: `servers`, `shards`,
`region_shards`, `player_servers`), seeded from `deploy/servers/<env>` and read by the gateway and
the world server. A per-region `data_shard` cannot host it.

`index` is the natural home by scope, but it is currently a *routing* directory and adding content
identity to it widens what that module means. See [B4](blockers.md#b4) — this is a placement call
with a deployment consequence, not a naming preference.
