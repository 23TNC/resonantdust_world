# Migrating to the reshaped leaf

Three moves, one pass ([README.md](README.md) Decided):

1. **Leaf reshape** — fold `dir`/`part` into the map filename, drop `<id>`.
2. **Drop `<subkind>`** — never a `definition_reference` field; taxonomy → `type/subtype/kind/variant`.
   Affects existing trees (`…/conifer/default/<v>` → `…/conifer/<v>`).
3. **Fold `linked/`→`biome-tile`** — material→`kind`, form→`variant`, `blueprint`→a `kind`; `kind_id`
   0x800-split ([VARIABLES.md](../../../../../VARIABLES.md#kind_id-partition--ground-tiles-vs-linked-objects-biome-tile)).

## The transform (leaf)

Per file — two shapes, since `linked/` also inverts kind↔material and folds to `biome-tile`:

```
scattered thing / plain tile:
FROM   <type>/<subtype>/<kind>/<subkind>/<id>.<dir>.<part>/<variant>/<map>.<ext>
TO     <type>/<subtype>/<kind>/<variant>/<map>.<dir>.<part>.<ext>
e.g.   biome-thing/default/conifer/default/1.e.0/3/albedo.png
   →   biome-thing/default/conifer/3/albedo.e.0.png

linked object:
FROM   linked/<form>.<material>/<id>.<dir>.<part>/<variant>/<map>.<ext>
TO     biome-tile/<biome>/<material>/<form>/<map>.<dir>.<part>.<ext>
e.g.   linked/wall.smooth/1.l.0/1/albedo.png
   →   biome-tile/default/smooth/wall/albedo.l.0.png
```

Mechanically: discard `<id>` and `<subkind>`; append `<dir>`/`<part>` to the map filename; `<variant>`
(or, for linked, the `<form>`) becomes the folder directly under `<kind>`. Multiple direction/part
folders that shared a variant **collapse into one folder** of co-located files. For `linked/`, split the
old `<form>.<material>` stem: **material → `<kind>`**, **form → the `<variant>` folder**, biome →
`<subtype>` (`default`); `blueprint` old-subkind becomes its own `kind`.

**Linked is a held-whole atlas, not per-cell.** The old per-cell `1.l.0/<1..16>/` numeric `<variant>`
folders are the earlier autotile split, now retired (README §7): a linked form is ONE atlas master + an
`atlas.json` (`grid`,`pad`) sidecar, client cell-by-UV. So the migration **drops** the per-cell
folders and carries `{albedo,normal,…}.l.0.<ext>` + `atlas.json`. The on-disk tree is **half-migrated**
(`wall.smooth` already an atlas; `wall.blueprint` still 16-split; `fence.*`/`rock.*` empty) — so
still-split/empty kinds must be **re-mastered** to an atlas, not renamed.

- **Scripted `mv` pass**, same discipline as the prior migration
  (`bin/lib/migrate_texpaths.py` is the precedent — dry-run table first, then `--apply`).
  `textures/` is gitignored, so the on-disk old tree is the only rollback — **copy, don't
  move, until verified**, then drop the old leaves.
- Watch the known-broken `wall.smooth` masters (double-encoded `1.l.0.l.0` stem, flagged in
  [`docs/texture-paths.md`](../texture-paths.md)) — re-master rather than blindly rename.

## Consumers that encode the leaf shape (must change together)

The leaf shape is written and read in several places; `bin/lib/texpath.py` is the **shape
source-of-truth** and should change first, the rest follow. Paths per the 0.2.2 restructure —
**verify each before editing**, some moved:

- **`bin/lib/texpath.py`** — the canonical path builder. Change the leaf composition here;
  it's re-implemented natively in `marigold/delight.py`, so mirror it there.
- **`bin/art` + `bin/lib/*.py`** (`split_layers.py`, `generate.py`, `emissive.py`,
  `marigold/delight.py`) — every write site + the manifest walk (`_kind_maps`, the
  variant/part counters) that globs the leaf.
- **Server texture resolvers** — `textures.rs` (`master_albedo_rel`) + `tex_manifest.rs`
  (`scan_masters`). These build/probe the leaf path; now `.../<id>/<variant>/albedo.<dir>.<part>.png`.
  ⚠️ Under 0.2.2 these likely live under `server/edge/` (content/texture serving moved there);
  confirm the current path.
- **Client** — `client/pixijs/src/textures/*` (`TextureResolver.ts`, `MaxRectsPacker.ts`,
  `previewCache`, `textureManifest`, and the `lod`/URL contract). The fetch URL + cache key
  gain `<dir>.<part>` in the filename. This overlaps the deferred **Phase 5 (client
  multi-map)** from the old plan — the two should land together.

## Order

1. ✅ **`linked → type/subtype` mapping** — resolved: `linked/`→`biome-tile`, material→`kind`
   (0x800-split), form→`variant`, biome→`subtype` ([README.md](README.md) Decided §5).
2. **`type/subtype` `meta.json`** ([README.md](README.md) Decided §6) — the kind registry
   (name → `kind_id`, so the tooling knows tile-vs-linked) + form → `variant_id`. `bin/art` needs it
   to place files and to split sheets. Author it before the write sites depend on it.
3. **`bin/lib/texpath.py`** — new leaf composition: `<id>`-free, `<subkind>`-free, `map.dir.part`
   filename, biome-tile fold (the SoT); mirror in `marigold/delight.py`.
4. **`bin/art` write + manifest** sites — every write + the manifest walk; the manifest truncates
   `variant_id ≥ 16`.
5. **Scripted `mv`/copy** migration of the existing tree (dry-run → apply): leaf reshape + drop
   `<subkind>` + `linked/→biome-tile` fold, one pass. **Copy, don't move, until verified** (`textures/`
   is gitignored → the old tree is the only rollback).
6. **Server resolvers**, then **client** fetch/cache contract (URL + cache key gain `.dir.part`,
   lose `<subkind>`).
7. **Verify end-to-end on the running stack** (`rd up` → `rd deploy` → browser renders a
   pawn/thing/wall) — this changes what the client fetches, so unit tests can't close it. Drop the
   old leaves once satisfied.

_No storage step: the dense tile vector is already `Vec<u16>` `kind_reference`, so linked
(`kind_id ≥ 0x800`) fits with no codec/worldgen/edge change._
