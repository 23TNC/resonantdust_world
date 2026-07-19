# Texture layout — the go-forward on-disk shape

This folder is the **current** design for how texture files are named and laid out, under the
0.2.3 object-model taxonomy. It supersedes [`docs/texture-paths.md`](../texture-paths.md),
which is accurate for the 0.1.x *folder-per-variant* work but predates the
`type/subtype/kind/subkind` taxonomy and the leaf reshape below — treat that file as history.

Read this before touching `bin/lib/texpath.py`, `bin/art`, `bin/lib/*.py`, `marigold/*.py`,
the server texture resolvers, or `client/pixijs/src/textures/*`. The migration plan is in
[migration.md](migration.md).

Status legend: ✅ AGREED ·
✏️ DRAFT (confirm) · ❓ OPEN · ⚠️ NOT YET (code doesn't match).

---

## The canonical path

```
textures / <type> / <subtype> / <kind> / <variant> / <map>.<dir>.<part>.<ext>
          └────── object-model taxonomy ──────┘ └ form/var ┘ └──── the leaf file ────┘
```

- ✅ **`<type>/<subtype>/<kind>/<variant>`** — the object-model taxonomy, the **exact four fields**
  of [`definition_reference`](../../../../../VARIABLES.md) (`type_id`/`subtype_id`/`kind_id`/`variant_id`).
  e.g. `pawn/animal/wolf/0/`, `biome-thing/default/conifer/3/`, `biome-tile/default/plank/wall/`.
  **No `<subkind>`** — it was never a `definition_reference` field (the old u4 was `variant_id`, not
  `subkind_id`); dropping it makes the folders match the id 1:1.
- ✅ **`<variant>`** — the `variant_id`. May be **named or numeric**: a scattered thing's art variations
  are numeric folders (`0..15`, straight off a sprite sheet); a linked object's are named **forms**
  (`wall`/`fence`/`rock`) that a registry maps to an index. Both resolve to `variant_id`. **≥ 16 truncates**
  out of the manifest (`u4`). **No `<id>` segment** (see below).
- ✅ **the leaf file `<map>.<dir>.<part>.<ext>`** — this is the change (below).

### No `<id>` segment ✅

`<id>` is **dropped.** It only existed to tag which sprite sheet a variation came from, which
is redundant:

- A per-variation `sprite` map already **lives in its `<variant>/` folder** alongside the
  other maps.
- A sprite **sheet that spans multiple variations** carries the variant **in its own filename
  + metadata** (the alias/meta file — the "three-level source model"), which is also what
  assigns `subkind` and the rest. So the sheet-of-origin is recoverable without a path
  segment.

Nothing needs a separate id level, so it's gone.

### Segment glossary

| segment | is | notes |
|---------|-----|-------|
| `type` / `subtype` / `kind` | object-model taxonomy | [VARIABLES.md](../../../../../VARIABLES.md); `type` reserved → dirs use these exact words. **No `subkind`** (not a field). For `biome-tile`: `subtype`=biome, `kind`=material/blueprint (0x800-split tile-vs-linked) |
| `variant` | `variant_id` (u4) | named (linked **form**: `wall`/`fence`/`rock`) or numeric (art variation `0..15`); a registry maps names → index; ≥16 truncates |
| `map` | the texture channel | `albedo` / `normal` / `diffuse` / `emissive` / `packed` / `sprite` / … |
| `dir` | facing / direction | ✅ **compact**: `s`/`e`/`n` (west = mirrored east), `l` = omni/linked. From object-model rotation (`rot`→`dir`) |
| `part` | sprite piece within the object | ✅ named **`part`** (not `layer` — `layer` is the object-model tile slot). Compact numeric: `0` body, `1` head, … |

---

## The change — reshape the leaf

We moved the upper path onto the taxonomy but **stopped before reshaping the leaf** (and before
dropping `subkind` / folding `linked/`). Today direction/part sit in a *folder* name, the map is a
bare file, and there's a redundant `<id>` + `<subkind>`:

```
CURRENT   …/<kind>/<subkind>/<id>.<dir>.<part>/<variant>/<map>.<ext>
real      textures/linked/wall.smooth/1.l.0/1/albedo.png
```

**Target** — drop `<id>` and `<subkind>`, fold `linked/`→`biome-tile` (material→`kind`, form→`variant`),
move `<dir>`/`<part>` into the map filename, and make `<variant>` the one folder that holds them all:

```
TARGET    <type>/<subtype>/<kind>/<variant>/<map>.<dir>.<part>.<ext>
real      biome-tile/default/smooth/wall/albedo.l.0.png   (was linked/wall.smooth/1.l.0/1/albedo.png)
          biome-thing/default/conifer/3/albedo.e.0.png     (was biome-thing/default/conifer/default/3/albedo.png)
```

### Why — one folder per variant, grouped by map → dir → part

The point is **readability**: put every direction and part of a variation in **one folder**,
and order the filename `map.dir.part` so a plain `ls` groups by map first, then direction,
then part. For a pawn variant with facings `s/e/n` and parts `0`=body / `1`=head:

```
<variant>/
  albedo.e.0.png    ← all albedos group…
  albedo.e.1.png
  albedo.n.0.png    ← …then by direction (e, n, s, w)…
  albedo.n.1.png
  albedo.s.0.png
  albedo.s.1.png    ← …then by part (0, 1)
  normal.e.0.png    ← …then the next map
  normal.e.1.png
  …
```

Today those live in separate `1.s.0/`, `1.e.0/`, `1.n.0/`, `1.s.1/`… folders, so you can't
see a whole variation at a glance. After the reshape, one `readdir` of `<variant>/` shows the
entire variation, sorted into map/direction/part blocks. Names stay **compact** (`s`/`e`/`n` +
numeric parts) — the `map.dir.part` **order** is what produces the grouping.

---

## Decided

1. ✅ **No `<id>`.** Dropped — redundant (sheet-of-origin lives in the variant folder / the
   multi-variant sheet's own filename + metadata). Path starts variations at `<variant>/`.
2. ✅ **Compact names.** Keep `s`/`e`/`n` + numeric parts; do **not** expand to full words.
3. ✅ **`part`, not `layer`.** `layer` stays reserved for the object-model tile slot.
4. ✅ **No `subkind`.** Dropped — never a `definition_reference` field (the u4 is `variant_id`).
   Taxonomy is the four fields `type/subtype/kind/variant`. Affects existing trees too:
   `biome-thing/default/conifer/default/<v>` → `biome-thing/default/conifer/<v>`.
5. ✅ **`linked/` folds into `biome-tile`.** Walls/fences/rocks/blueprints become `TYPE_BIOME_TILE`
   objects so they ride the dense tile vector — **no new `type_id`**. The old `<kind>.<subkind>`
   inverts: the **material** (old subkind: `plank`/`brick`/`smooth`/…) becomes `kind`; the **form**
   (old kind: `wall`/`fence`/`rock`) becomes `variant`. `blueprint` is a `kind` (N generic blueprints
   across the forms). `kind_id` is split at `0x800` (tile < 0x800 ≤ linked); see
   [VARIABLES.md](../../../../../VARIABLES.md#kind_id-partition--ground-tiles-vs-linked-objects-biome-tile).
   ```
   linked/wall.plank/1.l.0/<v>/albedo.png  →  biome-tile/default/plank/wall/albedo.l.0.png
   linked/rock.flecked/…                    →  biome-tile/default/flecked/rock/albedo.l.0.png
   ```
6. ✅ **A `type/subtype`-level `meta.json`.** Retained/reintroduced (we dropped the type/subtype sprite
   sheets + their split metadata). It's the spot that "tells art what it's looking at": the kind
   registry (name → `kind_id`, so the pipeline knows which materials are linked vs tile), the form →
   `variant_id` map, and sprite-sheet split info. Distinct from the **leaf** `meta.json` (per-variant
   outline/bbox/tints for shadows). Lives at `textures/<type>/<subtype>/meta.json`.
