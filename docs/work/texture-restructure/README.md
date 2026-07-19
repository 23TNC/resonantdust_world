# Work — texture-restructure (leaf reshape + drop subkind + linked→biome-tile)

_Opened 2026-07-19. Migrate the on-disk texture tree to the **agreed** go-forward shape. The shape is
already decided + authoritative — this stream is pure **execution**. Design:
[`texture-layout/`](../../components/dev/textures/design/texture-layout/README.md) (canonical path +
Decided) · [`migration.md`](../../components/dev/textures/design/texture-layout/migration.md) (the
transform + order) · packing in [`VARIABLES.md`](../../VARIABLES.md#kind_id-partition--ground-tiles-vs-linked-objects-biome-tile)._

## What changes

Three moves, one pass, applied to `textures/`:

1. **Leaf reshape** — `<id>.<dir>.<part>/<variant>/<map>.<ext>` → `<variant>/<map>.<dir>.<part>.<ext>`.
   Drop `<id>`; fold `dir`/`part` into the map filename; one `<variant>/` folder holds them all
   (`ls` groups by map→dir→part).
2. **Drop `<subkind>`** — never a `definition_reference` field (the u4 is `variant_id`). Taxonomy →
   the exact four fields `type/subtype/kind/variant`. Hits existing trees too:
   `biome-thing/default/conifer/default/<v>` → `biome-thing/default/conifer/<v>`.
3. **Fold `linked/`→`biome-tile`** — walls/fences/rocks/blueprints become `TYPE_BIOME_TILE` (ride the
   dense tile vector, no new `type_id`). The old `<form>.<material>` **inverts**: material→`kind`,
   form→`variant`, `blueprint`→a `kind`, biome→`subtype`. `kind_id` split at `0x800` (tile `<0x800`,
   linked `≥0x800`).

```
biome-thing/default/conifer/default/1.e.0/3/albedo.png  →  biome-thing/default/conifer/3/albedo.e.0.png
linked/wall.smooth/1.l.0/<atlas>/albedo.png             →  biome-tile/default/smooth/wall/albedo.l.0.png + atlas.json
```

**Linked = held-whole autotile atlas** (see [`issues.md`](issues.md) I1, [`forks.md` F3](forks.md)).
A linked kind is ONE atlas per form — a master + an `atlas.json` (`[cols,rows]`+`pad`) sidecar, the
client sampling a cell by UV — **not** per-cell `<variant>` folders (that `1..16` split is superseded).
So the linked leaf is `biome-tile/<biome>/<material>/<form>/{albedo,normal,…}.l.0.png` **+ `atlas.json`**.
The disk is **half-migrated** (`wall.smooth`=atlas, `wall.blueprint`=16-split, `fence`/`rock`=empty), so
P3 **re-masters** the still-split/empty kinds — it is not a uniform rename.

## Scope — FILE STRUCTURE only (storage already accommodates linked)

This stream migrates **texture files + the code that writes/reads their paths**. It needs **no
server-storage change**: the dense tile vector is **already `Vec<u16>` of `kind_reference`**
(`kind_id:12 | variant_id:4` — `action.rs` "tile = dense `kind_reference` by index"; `tile/*.rs`
`tiles: Vec::<u16>`), so a linked object is just a `kind_reference` with `kind_id ≥ 0x800` dropped into
the existing vector. That's the whole point of folding linked into biome-tile — it **already fits**. See
[`forks.md`](forks.md) F4.

## The chain (each encodes the leaf shape — must change together)

`bin/lib/texpath.py` is the **shape SoT** and changes first; the rest follow. Per
[`migration.md`](../../components/dev/textures/design/texture-layout/migration.md): a new
**`type/subtype` `meta.json`** registry (kind name→id + tile/linked flag, form→variant) precedes the
write sites — `bin/art` needs it to know which materials are linked and where files land.

- **`textures/<type>/<subtype>/meta.json`** — new registry (the "tell art what it's looking at" file).
- **`bin/lib/texpath.py`** (+ mirror in `marigold/delight.py`) — leaf composition.
- **`bin/art` + `bin/lib/*.py`** — write sites + the manifest walk (truncates `variant_id ≥ 16`).
- **Scripted copy** of the existing tree (dry-run → apply; `textures/` gitignored → copy, don't move).
- **Server resolvers** (`server/edge` `textures.rs`/`tex_manifest.rs`) — build/probe the new leaf.
- **Client** (`client/pixijs/src/textures/*`) — fetch URL + cache key gain `.dir.part`, lose `<subkind>`.

## State

Phased in [`todo.md`](todo.md); decisions in [`forks.md`](forks.md); human-input in
[`blockers.md`](blockers.md). The shape itself is **not** re-litigated here — it's Decided in the
design docs; this stream only carries it out + records execution snags.
