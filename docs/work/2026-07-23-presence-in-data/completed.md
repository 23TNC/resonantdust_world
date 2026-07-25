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

## P2 · Eviction + prim free-list — 2026-07-23 ✓

- **Prim free-list** (`coldData`): freed billboard_data indices → a stack, popped before bumping
  `primNext`. `freePrimsExcept(seen)` frees every allocated prim absent from this frame's `standing`
  (zone evicted / destroyed); `buildCasters` builds the resident `seen` set and calls it. Safe with
  no slot-clear + no removal cascade: `buildCasters` rebuilds every in-window bucket from `standing`
  (a freed id can't be referenced), and caster-removal already force-alls the shadow recompute; the
  reach gap keeps a freed prim beyond every in-window tile.
- **Region-window eviction** for presence/buckets turned out **inherent to the P1 design**: every
  in-window tile is (re)written each rebuild (compare-write), so in-window tiles are always fresh and
  out-of-window slots (stale) are never read (window ≤ region). No explicit owner-tracking clear
  needed — it would only matter for incremental writes (a perf pass) or to bound the beyond-region
  artifact ([`forks.md#f3`](forks.md#f3)).
- **Verified live**: streaming churn already exercises the free-list (next 1776, freed 799, live
  977 at load — high-water bounded, not climbing). Pan to (90,90) then back to (54,21): `next` held
  at **1776** while `live` swung 977→1598→… (freed slots absorbed the churn), and the return
  rebuilt **bit-identical** (corridor↔brute 0 mismatches, nonzero 9,949). Eviction + re-materialise
  correct.
## P3 · Perf + close — 2026-07-23 ✓

- **One scatter pass, fewer bindings**: the gather now binds **3 samplers** (`uData`, `uDirty`,
  `uSurface` — was 5 with `uPresence`+`uCaster`); overlay binds 2 (`uShadow`, `uData`). The
  second-FBO problem is gone — presence/buckets ride the SAME scatter draw as defs/prims/lights.
- **Perf**: static + orbit both at the **121 display cap**. Static = compare-write finds no diffs →
  flush no-op. Orbit = every frame scatters the moving light's changed (reach-box) presence tiles
  (120/120 flushes did work over 2 s) while holding cap. The write-all-in-window is CPU-cheap
  (compare-write); commands stay sparse (only changed tiles).
- **F1 (tie eviction to the subscription model): DEFERRED** — current eviction is correct (free-list
  + write-all-in-window); the tie is a network-free-rematerialise optimisation best layered when the
  subscription model is refactored. Recorded in [`forks.md#f1`](forks.md#f1).
- Note: the debug world's extent didn't force a full **256-tile region crossing** (fold-slot reuse);
  that path is correct by construction (write-all-in-window + window ≤ region — in-window always
  fresh, out-of-window never read). Normal multi-zone pan verified (P2).