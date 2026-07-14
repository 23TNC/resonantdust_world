# Texture layout — the go-forward on-disk shape

This folder is the **current** design for how texture files are named and laid out, under the
0.2.3 object-model taxonomy. It supersedes [`docs/texture-paths.md`](../texture-paths.md),
which is accurate for the 0.1.x *folder-per-variant* work but predates the
`type/subtype/kind/subkind` taxonomy and the leaf reshape below — treat that file as history.

Read this before touching `bin/lib/texpath.py`, `bin/art`, `bin/lib/*.py`, `marigold/*.py`,
the server texture resolvers, or `client/pixijs/src/textures/*`. The migration plan is in
[migration.md](migration.md).

Status legend (same as [`docs/spacetime-tables/`](../components/server/spacetime/modules/shard/design/README.md)): ✅ AGREED ·
✏️ DRAFT (confirm) · ❓ OPEN · ⚠️ NOT YET (code doesn't match).

---

## The canonical path

```
textures / <type> / <subtype> / <kind> / <subkind> / <variant> / <map>.<dir>.<part>.<ext>
          └───────── object-model taxonomy ─────────┘ └ variation ┘ └──── the leaf file ────┘
```

- ✅ **`<type>/<subtype>/<kind>/<subkind>`** — the object-model taxonomy, the same axes as
  [`object_type_reference` + `object_kind_reference`](../components/shared/codec/design/references/object-reference.md)
  (`type_id`/`subtype_id`/`kind_id`/`subkind_id`). e.g. `pawn/animal/wolf/…`,
  `biome-thing/default/berry/…`.
- ✅ **`<variant>`** — the variation index (`variant_id`) — one folder per variation, the
  first level directly under the taxonomy. **No `<id>` segment** (see below).
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
| `type` / `subtype` / `kind` / `subkind` | object-model taxonomy | [object-reference.md](../components/shared/codec/design/references/object-reference.md); `type` reserved → dirs use these exact words |
| `variant` | variation index | the `variant_id`; a `readdir` enumerates variations |
| `map` | the texture channel | `albedo` / `normal` / `diffuse` / `emissive` / `packed` / `sprite` / … |
| `dir` | facing / direction | ✅ **compact**: `s`/`e`/`n` (west = mirrored east), `l` = omni/linked. From object-model rotation (`rot`→`dir`) |
| `part` | sprite piece within the object | ✅ named **`part`** (not `layer` — `layer` is the object-model tile slot). Compact numeric: `0` body, `1` head, … |

---

## The change — reshape the leaf

We moved the upper path onto `type/subtype/kind/subkind` but **stopped before reshaping the
leaf**. Today the direction/part sit in a *folder* name and the map is a bare file:

```
CURRENT   …/<kind>/<subkind>/<id>.<dir>.<part>/<variant>/<map>.<ext>
real      textures/linked/wall.smooth/1.l.0/1/albedo.png
```

**Target** — drop `<id>`, move `<dir>` and `<part>` off the folder into the map filename, and
make `<variant>` the folder that holds them all:

```
TARGET    …/<kind>/<subkind>/<variant>/<map>.<dir>.<part>.<ext>
real      textures/linked/wall.smooth/1/albedo.l.0.png
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
4. ✅ **Finish the taxonomy move.** `linked/…` (still legacy `<category>/<kind>.<subkind>`)
   moves onto `<type>/<subtype>/<kind>/<subkind>` as part of this pass — see
   [migration.md](migration.md).

### Still to pin (mechanical, not blocking the shape)

- ❓ **`linked/…` → taxonomy mapping.** Which `<type>/<subtype>` do walls/fences/rocks land
   under? The object-model `type_id` palette has no "built/linked" type yet
   ([object-reference.md](../components/shared/codec/design/references/object-reference.md)) — needs a type assignment (or a
   `layer`-on-`biome-tile` decision) before `linked/` can be relocated.
