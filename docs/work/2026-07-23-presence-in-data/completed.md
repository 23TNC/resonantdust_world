# Completed — presence + buckets into the data texture

_Done **and** verified (identity diff + zoom round-trip + region-crossing eviction). Items move here
from [`todo.md`](todo.md). Append-only history; authoritative for what's done._

---

## P0 · Layouts — 2026-07-23 ✓

- `docs/VARIABLES.md`: sets 3 (light_presence) + 4 (caster_buckets), the region-torus zone-strip
  fold, the 7-slot self-addressing px shape, and the **DATA = persistent** rule (dirty stays out).

## P1 · Fold presence + buckets into the data texture (region-torus) — 2026-07-23 ✓

- `coldData`: `PRESENCE_BASE`/`CASTER_BASE` (sets 3/4), `foldTile(wc,wr)` (zone-strip, matches GLSL),
  `writePresence`/`writeCasters`/`clearPresence`/`clearCasters` — compare-write the self-addressing
  texel (u16 id in R high + 7 slots) and mark → scatter. Separate `presenceTex`/`casterTex` retired.
- `buildPresence`/`buildCasters`: nearest-7 / ≤7-per-tile into a dense window-local scratch, then
  write EVERY in-window tile via coldData (compare-write diffs empty↔populated). Slot loops 8→7.
- Gather + corridor + brute + overlay: read presence/buckets via `fetchLin(uData, BASE + foldTile())`
  — the corridor walk moves from window-slot space to the fixed region-torus (map-model rigidity).
  GLSL `foldTile`/`tileSlot` use static lane selection (no dynamic subscript — the ANGLE foot-gun).
  `uPresence`/`uCaster` bindings dropped; overlay binds `uData`.
- **Verified**: corridor↔brute **0 mismatches**; nonzero **exactly 9,949** (bit-identical to
  pre-migration — 7 slots didn't drop anything in-scene); zoom in→out holds identity + silhouettes;
  ONE scatter pass, no second FBO; console clean.
