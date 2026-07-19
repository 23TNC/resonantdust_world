# Completed — texture-restructure

_Done + verified. Items move here from [`todo.md`](todo.md) (`todo → completed`). Append-only history;
authoritative for what's actually shipped. Timestamp each row with the date it landed._

---

- **2026-07-19** · **P1 (part) — `texpath.py` leaf reshape.** Rewrote [`bin/lib/texpath.py`](../../../bin/lib/texpath.py)
  to the new leaf `<variant>/<map>.<dir>.<part>.<ext>` — drop `<id>` + `<subkind>`; `<part>` replaces the
  old `<layer>` filename segment; added `parse_map` + `leaf_file` (per-variant `meta.json`/`atlas.json`,
  which carry no dir/part, distinct from directional `sibling`). Updated consumers: `generate.py`
  (`variant_leaf`/`map_name`), `meta.py` (`meta.json` via `leaf_file`). Deleted the dead
  `bin/lib/migrate_texpaths.py` (0.1→0.2 tool, done 2026-07-04; git holds it; precedent mentions →
  git history). **Unit-tested** the new API (round-trips, `find_maps`, `sibling` vs `leaf_file`); all
  five consumers import clean.

- **2026-07-19** · **P1 (part) — `marigold/delight.py` mirror.** Mirrored the new leaf natively in the
  marigold venv: `find_diffuse` globs `diffuse.<dir>.<part>.png`, `out_path` emits `<target>.<dir>.<part>.png`
  at the same dir/part (+ `_diffuse_dir_part`/`_is_diffuse` helpers). `find_diffuse`/`out_path` signatures
  unchanged, so `normals.py`/`depth.py`/`normal_depth.py` are untouched. Syntax-checked all four; verified
  the path logic **agrees with `texpath.py`**. **Remaining in P1:** only the biome-tile fold in texpath
  (sequenced after the P0 registry).

- **2026-07-19** · **P3/P4/P5/P6 — clean-case cutover, BROWSER-VERIFIED.** For the `subkind==default`
  kinds (biome-thing + wolf):
  - **P3** [`bin/lib/migrate_leaf.py`](../../../bin/lib/migrate_leaf.py) reshaped 573 files on disk
    (`<kind>/default/<id>.<dir>.<part>/<variant>/<map>.<ext>` → `<kind>/<variant>/<map>.<dir>.<part>.<ext>`);
    skipped human-pawn body-type subkinds + linked (phase 2). Caught + fixed a prune bug (the `default`
    *subtype* collided with the subkind) via immediate on-disk verification.
  - **P4** edge readers flipped: `master_map_rel` → `1/<map>.<facing>.0.png`; `register_kind` + `leaf_hash`
    scan the variant leaf `1/` with the facing in the filename. Compiled + redeployed.
  - **P5** DSL stems dropped the trailing `/default` (conifer/flora/wolf); the client is stem-based (no change).
  - **P6** verified in the browser (Chrome ext, debug scene): conifer + flora **render** from the migrated
    new-leaf textures; the edge serves the subkind-less stems 200 for albedo/normal/surface/layers
    (`GET /textures/lod/<hash>/256/<map>/biome-thing/default/conifer/e` → 200). End-to-end works.

- **2026-07-19** · **Linked leaf reshape + regression fix — verified.** Phase 1 had regressed linked
  rendering (see [`issues.md`](issues.md)); reshaped `linked/` too (generalized `migrate_leaf.py` to the
  no-subkind layout). Verified via the client's own texture path (JS in the browser): `linked/wall.smooth/l`
  is in the manifest with **`grid: [4,4]`** (held-whole 16-cell autotile atlas — the `1..16` split was of
  *this* atlas, confirming F3), the LOD serves 200 and **decodes to a 256×256 ImageBitmap**. So the leaf
  reshape is done + verified for **all existing texture kinds** with a render target. **Still phase 2:**
  the full biome-tile **fold** (material→kind rename; object-model-coupled, walls unplaced), `bin/art`
  regeneration (offline), and human-pawn body-type subkinds (a modeling question — no `body-type` def field).

- **2026-07-19** · **R1 — human-pawn body-types folded (user's call).** Body-type is **not** dropped or
  crammed into the variant space — it **folds into the kind name** `<kind>.<subkind>` (a distinct kind),
  matching the `linked/<form>.<material>` dotted-kind convention. Generalized `migrate_leaf.py` to fold
  any non-`default` subkind (drop `default`) + clean up emptied `<kind>/` dirs; reshaped 183 files:
  `pawn/human/{dead,male,female}/{average,thin,fat,fit}/…` → `…/male.average/<variant>/<map>.<dir>.<part>`
  etc. Verified: the dotted human kinds serve 200 (`pawn/human/male.average/{e,s,n}`) + appear in the
  manifest. (No browser render — humans aren't worldgen-placed — but the serving path is identical to the
  rendered conifer/wolf.) `dead.thin` etc. that carry only a `template` (no albedo) aren't stems yet, as
  expected. **The leaf reshape is now complete for every existing texture kind.**

- **2026-07-19** · **R3 (core) — `bin/art` cut over to the new leaf.** The migration had broken
  `bin/art` (its `_find_diffuse`/`_map_path` looked for bare `diffuse.png`/`<map>.png`). Cut over the
  read/derive helpers (`diffuse.*.png`; `_map_path` preserves the `<dir>.<part>` suffix), both remaster
  write sites (grid atlas → `<kind>/<id>/<map>.l.0.png` + `atlas.json`; non-grid → reads `sprite.*.png`,
  writes `diffuse.<dir>.<part>.png`), and the existence/map-bit checks + map-scaler exclusions
  (`<map>.png` → `<map>.*.png`). Verified: `bash -n` clean; `_find_diffuse` finds the migrated
  grid/non-grid/dotted-kind trees (was 0). **Remaining (flagged in-code, non-blocking):** the
  `bin/art manifest` variant/layer counters (`_id_variant_pairs`/`_layer_count`) still walk the old
  `<id>.<dir>.<layer>` pose dirs — they only regenerate `content/visual/manifest/*.rd`, which nothing
  renders off (the edge builds its own runtime manifest). Reworking that to the new model is the last piece.

- **2026-07-19** · **R2 — linked → biome-tile fold + rock revert (user-directed).** Two moves:
  - **Reverted the `rock` thing to a gray-square primitive** (`content/visual/things.rd`): dropped the
    `"linked/wall.smooth" &thing.texture set` stand-in (it had borrowed the wall material because it was
    the only mastered linked kind) — now `"white"` + a gray tint. Rock was a placeholder; the wall
    material belongs to walls, not rock.
  - **Folded `linked/` → `biome-tile/`** (`bin/lib/migrate_fold.py`, 48 files): material→kind, form→variant,
    biome→`default` (`linked/<form>.<material>/…` → `biome-tile/default/<material>/<form>/…`). `wall.smooth`'s
    held-whole atlas folded cleanly (`biome-tile/default/smooth/wall/albedo.l.0.png` + `atlas.json`); the
    10 un-mastered kinds (superseded 16-split `wall.blueprint`, raw sources) were **moved, not dropped** —
    preserved under their new home for a later `bin/art` re-master. `linked/` removed.
  - **Verified** (edge redeploy + browser): no wall/linked stems in the manifest (folded away), scene
    renders, rocks now draw as gray squares, no console errors.
  **Remaining (tied to wall placement, not this stream):** the edge/DSL/client resolution of biome-tile
  **named-variant** linked stems + re-mastering the un-mastered kinds — both only matter once the
  biome-tile object model actually places walls/fences/rocks as tiles.
