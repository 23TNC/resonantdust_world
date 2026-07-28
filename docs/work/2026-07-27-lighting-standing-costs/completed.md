# Completed — lighting standing costs

_Dated evidence: what landed and how it was verified._

## 2026-07-27 · P0 — the standing tax measured: 0.21–0.23 ms/frame

Harness: `EXT_disjoint_timer_query_webgl2`, one `TIME_ELAPSED` query per draw/blit, categorised by
render target (`o.target === coldShadowRT` etc.) and, for blits, by the bound `DRAW_FRAMEBUFFER`.
**Frames were pumped synchronously** (`__viewport.tick()` in a loop) because the tab was hidden and
Chrome freezes rAF — GPU timer queries need no presentation, so this is valid for GPU-side cost;
240 frames per run, world fully baked, 0 carried lights in view, `dirtyHist = {0: 240}` (every
frame fully clean).

Baseline (pre-P1, stashed edits, verified `debugClassDraws` absent), two runs:

| category | run 1 (ms/frame) | run 2 |
|---|---|---|
| blitCold + blitHot | 0.0345 | 0.0313 |
| gatherCold + gatherHot | 0.0955 | 0.0889 |
| lightCold + lightHot | 0.1014 | 0.0928 |
| **total standing tax** | **0.2314** | **0.2130** |

All six submissions fired on all 240 frames of both runs (`submitted: 240` each). ~8 % run-to-run
spread — noisier than the paced orbit harness, fine for a figure about to drop to zero.

## 2026-07-27 · P1 — dead cost deleted; standing tax now 0.000 ms

All three changes in `shadowGather.ts`, type-check green (`npx tsc --noEmit` exit 0):

1. **Per-class skip** — `buildDirty` now tallies `coldDirtyCount`/`hotDirtyCount`; `tick` skips a
   class's entire `classPass` at 0. `debugClassDraws` (0–2) instruments it.
2. **Prev-blit gated** — `DIFFERENTIAL_WIRED = false` module const; the snapshot blit runs only when
   it flips. Comment names the differential as the re-enabler.
3. **`oCasterD` deleted** — GATHER_FRAG is single-output; all four shadow RTs single-attachment
   (`rgba32uint`), second clear removed. `walkShadow`'s `cdepth` out survives for a future consumer.

**Verified, same pumped-frame environment:**

- **Static:** 240/240 frames → `drawHist {0: 240}`, `submitted {}` — zero draws, zero blits.
  Standing tax **0.2314 → 0.000 ms** by construction (nothing submitted to time).
- **Scoped path intact:** `markPrimDirty` on the torch carrier → that frame `dirty 144, draws 2`,
  next frames `0/0`. A placed torch registers (`carriedLights 1`) and lights the world (screenshot:
  warm pool + tree shadows).
- **Blit gate:** wrapped `gl.blitFramebuffer` counting calls, then `rebakeAll()` + 30 dirty ticks →
  **0 blits**.
- **Attachment deletion:** `coldShadowRT.textures.length === 1`; `drawOverlay` runs with no GL error
  (`getError() === 0`).
- **Identity, bit-exact vs baseline:** FNV-1a over the full cold shadow RT readback, torch at tile
  (15,−1) reach 8, identical procedure on both builds: brute hash **2575314166**, corridor hash
  **2077624216** — **P1 = baseline exactly, for both walks**. (The two walks differ from EACH OTHER
  by 398 words — pre-existing on the baseline, byte-identical counts; recorded as
  [issues I1](issues.md#i1), NOT introduced here.)

**Reach sweep, P1 build** (torch on a standing carrier, orbit, 240 frames, cold class does the
work; cleaner repeat runs for 4/12):

| reach | dirty/frame | gatherCold | lightCold | cold total | I2 table |
|---|---|---|---|---|---|
| 4 | 143.5 | 0.182 | 0.176 | 0.358 | 0.170 |
| 8 | 272.3 | 0.203 | 0.195 | **0.398** | **0.394** |
| 12 | 399.5 | 0.289 | 0.256 | 0.545 | 0.692 |

Reach 8 matches I2 to 1 % — **P1 does not change the moving-light marginal cost**, as designed.
Reach 4/12 deltas are environmental, not P1: this fixture differs from I2's (torch on a billboard
carrier adds prim rects — 143 dirty vs I2's 110 at reach 4 — different world location/density, and
pumped rather than paced frames). The per-frame win while MOVING is the hot-class side: gatherHot
fell to 0.006–0.010 ms (was a fullscreen discard draw) and both blits are gone.

## 2026-07-27 · P2 — dirty-rect draws; the per-texel gate and the dirty textures are gone

`buildDirty` now greedy-row-merges each class's mirror into tile-aligned rects emitted as raw NDC
triangles (6 verts/rect, non-indexed, one shared vertex list for both RT resolutions — tile
fractions are resolution-independent and cols/rows are pow2, so every rect edge is an exact dyadic
NDC coordinate on a texel boundary). `classPass` draws that geometry instead of the fullscreen
quad. The `uDirty` textures, their uploads, and both shaders' per-texel discard gates are DELETED —
clean texels persist by never being rasterized.

**Verified (pumped-frame environment as P0/P1):**

- **Rect exactness:** across a 60-frame orbit, `debugRectTiles == [coldDirtyCount, hotDirtyCount]`
  on every frame (`rectOk: true`); 144–160 dirty tiles merged into ~16 rects.
- **Bit-identity, twice:** the exact P1 fixture (fresh load → `__torch()` → corridor/brute
  readback hashes) reproduces **brute 2575314166 / corridor 2077624216** on the rect build WITH the
  gate still in, and AGAIN after the gate + dirty textures were deleted. Rasterized set == dirty
  set, proven at the output.
- **Static silence intact:** `debugClassDraws 0`, `getError() 0`.
- **Zoom sweep (the recurring drift class):** 1 → 0.5 → 0.25 → 1 with the torch lit — dims-change
  reallocation (rect arrays + geometry) exercised, no GL errors, dirty settles to 0, screenshot
  identical to pre-P2 (torch pool + tree shadows).
- **Sweep on the final build** (cold totals): reach 4 **0.382**, 8 **0.398**, 12 **0.486** ms.
  vs P1 fullscreen: the FINE draw (`lightCold`) improved at every reach (0.176→0.161, 0.195→0.182,
  0.256→0.214 — −9 %/−7 %/−16 %); the coarse gather is within this harness's ±8 % noise (its RT is
  512×256, so its discard tax was small). `rectTilesPerFrame` tracks `dirtyPerFrame` + ~8 (hot
  overlap double-count, expected). **Discard-tax share is now 0 by construction** — before, a
  reach-4 frame rasterized 512 tiles of fine fragments for 143 dirty (72 % discard-only).
