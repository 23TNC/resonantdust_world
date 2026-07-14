# Migrating to the reshaped leaf

Two moves, ideally one pass ([README.md](README.md) Decided):

1. **Leaf reshape** — `<id>.<dir>.<part>/<variant>/<map>.<ext>` → `<variant>/<map>.<dir>.<part>.<ext>`
   (drop `<id>`, fold `dir`/`part` into the map filename).
2. **Finish the taxonomy move** — relocate the still-legacy `linked/<kind>.<subkind>/…` trees
   onto `<type>/<subtype>/<kind>/<subkind>/…` (blocked on the `linked → type/subtype` mapping,
   README Decided §"Still to pin").

## The transform (leaf)

Per file:

```
FROM   …/<kind>/<subkind>/<id>.<dir>.<part>/<variant>/<map>.<ext>
TO     …/<kind>/<subkind>/<variant>/<map>.<dir>.<part>.<ext>

e.g.   linked/wall.smooth/1.l.0/1/albedo.png
   →   linked/wall.smooth/1/albedo.l.0.png
```

Mechanically: split the `<id>.<dir>.<part>` folder stem; **discard `<id>`**, append `<dir>`
and `<part>` to the map filename (`<map>.<dir>.<part>.<ext>`); `<variant>` becomes the folder
directly under `<kind>/<subkind>`. Multiple direction/part folders that shared a `<variant>`
**collapse into one `<variant>/` folder** of co-located files. (Since `<id>` was almost always
`1`, dropping it just removes a redundant level — but confirm no kind actually used a second
id before discarding.)

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

1. **Pin the `linked → type/subtype` mapping** (README §"Still to pin") — the only remaining
   unknown; everything else in the shape is Decided.
2. **`bin/lib/texpath.py`** — new leaf composition, `<id>`-free (the SoT), mirror in
   `marigold/delight.py`.
3. **`bin/art` write + manifest** sites.
4. **Scripted `mv`/copy** migration of the existing tree (dry-run → apply): the leaf reshape +
   the `linked/…` taxonomy relocation, in one pass.
5. **Server resolvers**, then **client** fetch/cache contract.
6. **Verify end-to-end on the running stack** (`rd up` → `rd deploy` → browser renders a
   pawn/thing) — this changes what the client fetches, so unit tests can't close it. Drop the
   old leaves once satisfied.
