# Definition registry — the corpus describes, the server numbers — 2026-08-04

_Components: `content/`, `shared/content`, `shared/codec`, `server/spacetime/modules` (the registry
table), `server/edge` (load + serve), `server/worker`, `shared/wasm`, `client/webgl`, `client/npc`.
Plan in [`todo.md`](todo.md); decisions in [`forks.md`](forks.md); measurements in
[`issues.md`](issues.md); what still needs the user in [`blockers.md`](blockers.md)._

## The user's call

> "If we just add type subType and kind to our toml, they self describe." — 2026-08-04
>
> "I see no reason to even hold numbers. On init our server can generate the manifest, we can go so
> far as to load it into a spacetime table… At which point we should be able to perform string→id
> operations client side." — 2026-08-04

## The problem, stated as it actually appears in the code

The wire's identity for a definition is already the taxonomy —
[`definition_reference : u32 = type_id:4 | subtype_id:12 | kind_id:12 | variant_id:4`](../../../shared/codec/src/object.rs).
But the corpus authors only `id = N` (the `kind_id`). The other three coordinates are **recovered by
string-parsing an art path** ([`npc/lib.rs:243`](../../../client/npc/src/lib.rs)):

```rust
let stem = visual_for_object(kind).texture;    // "pawn/animal/wolf"
let species_name = stem.split('/').nth(1)?;    // "animal"   ← taxonomy from a file path
let species = pawn_species_subtype_id(name)?;  // 1          ← a code-owned palette
let def = pack_definition_from_ids(TYPE_PAWN, species, kind, 0);   // TYPE_PAWN hardcoded
```

The codec states the coupling as a fact to rely on — *"the folder taxonomy and the definition fields
are 1:1"*. This stream makes that relationship **authored instead of inferred**, and then the art
path derives from the taxonomy rather than the taxonomy from the art path.

## The shape

**The corpus describes; the server numbers.** A definition authors `type` / `kind` plus
**applicability arrays** over the axes it spans ([F2](forks.md#f2)) — `subType = [forest, plains,
grassland]`, `variant = [0..15]` — so one conifer block covers 48 tuples instead of 48 blocks, while
a wall can still pin a single subType. At load the server cross-products the arrays and
allocates one `u32` id per `(type, subType, kind, variant)` tuple into a spacetime table:

```
u32 id | u32 version | string type | string subType | string kind | string variant
```

Clients receive the table on load and updates on change, and resolve `string → id` locally
([F9](forks.md#f9)). The corpus stops carrying numbers entirely — **superseding
[`toml-content` F1](../2026-08-04-toml-content/forks.md#f1)**, which made ids explicit *because the
loader owned identity*. It stops being right the moment a registry owns it.

## Versioning is the point, and the policy is deliberate

Change any field of a definition and its `version` bumps, minting a **new id**; the old row stays.
Resolution is *match the four strings, take the highest version* — so **new** placements get the new
definition and **existing objects keep the old id forever** ([F6](forks.md#f6)). There is no
migration sweep, and that is the feature, not a gap. The user's reasoning, verbatim in substance:

> "This prevents breaking the entire game when we make an update… what if the new apples expire in 1
> hour, but the old apples expire in 2 hours, what do we do with apples with 30 minutes remaining?
> What if we change the weight of apples from 1 to 2 — now all inventories with apples might be over
> capacity? But if all old apples are old apples and function like old apples, I don't think we would
> have many issues."

An old apple stays an old apple: same expiry, same weight, same behaviour, until it is spent.

## The constraint that shapes everything else

**The id stays STRUCTURED** ([F3](forks.md#f3)). It cannot become an opaque handle, because three
hot paths unpack its bits with no table available ([I1](issues.md#i1)) — and one of them is a
safety property: the worker routes `CREATE` on `def_type_id` and **rejects an unknown type by name
rather than silently pawning it**, inside the sim, per event.

So `version` is a table column for *resolution*, never an id field: a bump allocates a **fresh
coordinate**, and that coordinate is burned in **`kind`**, not `subType` ([F5](forks.md#f5)) —
`conifer.1` isolates the change to conifer, where `forest.1` would version every kind in the biome.

## What this deletes

- the stem-parsing in `client/npc` (taxonomy from a file path)
- the code-owned species palette in [`object.rs:120`](../../../shared/codec/src/object.rs), whose own
  comment says it can move to content *"once a content registry guarantees append-only numbering"*
- [`is_append_compatible_with`](../../../server/edge/src/worldgen.rs) — the hot-reload guard that
  refuses a corpus reorder because stored zones would misread. Ids stop coming from corpus order, so
  reordering becomes harmless and the guard is dead weight.

## What it does NOT replace

**The texture manifest.** Two different questions: *what objects exist and what are their ids* (the
registry) versus *which image files exist under a path* (a disk scan). Authoring the taxonomy makes
the **path** derivable; it cannot make the path's **contents** knowable — that the wolf has 15
variants, which facings and maps are present, the content hash, the packed square. See
[`components/server/edge/intent/texture-index.md`](../../components/server/edge/intent/texture-index.md).
