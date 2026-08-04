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

## I2 — subtype IS decoded at runtime — from the COLD ROW, not from the def {#i2}

_Corrected 2026-08-04 after the user challenged the first wording, which was misleadingly broad._

The narrow, accurate fact: **`def_subtype_id` — the copy of subtype inside a `definition_reference`
— has no runtime reader.** The npc packs species into it via `pack_definition_from_ids` and nothing
unpacks it.

But subtype itself is very much live. It is read from the **cold row header** (`cold_row_subtype`)
at four sites in the worker ([main.rs:380/395/431/450](../../../server/worker/src/main.rs)), feeding
`subtype_id:` into shard writes, and it is *written* from content in two places —
`biome_subtype_id` for a cell's biome ([worldgen.rs:119](../../../server/edge/src/worldgen.rs)) and
`pawn_species_subtype_id` for a pawn's species.

So the earlier framing — "12 bits written and never decoded, possibly free space" — described one
accessor and implied the axis. It is the def's *copy* that is currently redundant with the row, not
the concept. And the user's objection stands independently of today's call sites: `pawn/human` and
`pawn/animal` are a real distinction, and nothing can ask it without enumerating kinds unless the
def carries it. **Subtype stays in the def** ([F10](forks.md#f10)).

## I3 — `variant_id` is u4 (16 slots) and the wolf already holds 15 {#i3}

From the live art manifest:

```
pawn/animal/wolf var_variant:
  [1, 124, 125, 555, 777, 888, 7001, 7002, 8101, 8102, 8201, 8202, 8301, 8302, 1682748910]   → 15
```

Those are the art tree's variant **labels**, not slots — one is ten digits. The registry handles that
(`string variant` in the row, u4 slot in the id), so labels are free. What is not free is the
**count**: `VARIANTS_PER_DEF = 16` and one kind is at 15 of 16 **today**. This is a live ceiling, not
a future one. With subtype STAYING in the def ([F10](forks.md#f10)), widening variant means
narrowing another field rather than reclaiming a dead one — see [B2](blockers.md#b2), which is now
the only open schema question.

## I4 — the taxonomy IS uniform; the stems are prefixes of varying depth {#i4}

_Corrected 2026-08-04. The original row claimed the stems were irregular and needed a ruling. They
are not, and it did not — I was reading the corpus without the art tree beside it._

`texture` in the corpus is the `type/subType/kind` PREFIX; the variant (and then the direction) are
appended when the stem is resolved against the art tree, which is why the live manifest holds
`biome-thing/default/conifer/0/e` while the corpus holds `biome-thing/default/conifer`.

So `biome-tile/default/smooth/wall` is not a fourth kind of thing: it is
`type=biome-tile, subType=default, kind=smooth, variant=wall` — a def that pins its variant because
a linked autotile's forms (`wall`/`fence`/`rock`) sit exactly where numeric variants sit. Uniform,
four axes, no exception.

`white` is likewise not a taxonomy question. It is the built-in no-texture fill —
`loader.rs:85` calls it the built-in `"white"` fill, the client compares `stem === "white"` to skip
linked resolution, `def_span.py` skips it as "not a texture tree path", and **no `white` file exists
anywhere under `textures/`**. It is already the marker for "this def has no art", which is what my
proposed `art = none` would have renamed it to. Orthogonal to the taxonomy; nothing to decide.

## I5 — the registry must be GLOBAL, but shards are per-region {#i5}

`definition_reference`s appear in every shard's stored rows, so the registry is world-global by
definition. The existing per-env global DB is `index` (routing: `servers`, `shards`,
`region_shards`, `player_servers`), seeded from `deploy/servers/<env>` and read by the gateway and
the world server. A per-region `data_shard` cannot host it.

**Resolved 2026-08-04 by the user** ([F11](forks.md#f11)): the registry lives in `index` and the
**master** allocates, being the single one. My objection — that `index` is a routing directory and
content identity widens it — was rebutted on the facts: the modules are split for **scaling**
(data shards can grow independently), not as a lifecycle division, and per-env global data has no
reason not to share a database. A sibling module stays available if the registry later grows its own
reducers, exactly as `chat`/`players` sit beside `index` today.
