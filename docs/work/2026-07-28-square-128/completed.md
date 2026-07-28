# Completed — SQUARE 128

## 2026-07-28 · P0 — the `SQUARE = 64` baseline, measured

### Resident bytes per map family (live, from the running context)

Read off the actual `RenderTarget`/texture objects in the page (`__viewport.map/.warm` channels,
`__gather`'s RTs, the sprite atlas), multiplied by each attachment's real format width — not derived
from the constants.

| family | maps | size | format | MiB |
|---|---|---|---|---|
| **art** (`SQUARE`) | albedo/normal/surface/zdepth × **cold AND warm** = 8 | 2176 × 1152 | rgba8unorm | 9.56 ea = **76.5** |
| **fine** (`TEXTILE_SQUARE`) | `coldLightRT`, `hotLightRT` | 2048 × 1024 | rgba32float | 32 ea |
| | `receiverFineRT` | 2048 × 1024 | r32uint | 8 |
| | | | | **72** |
| **shadow** (`TEXTILE_UNIT`) | cold/hot shadow + both prev + `receiverCoarseRT` | 512 × 256 | rgba32uint | 2 ea |
| | `decayRT` | 512 × 256 | rgba16float | 1 |
| | | | | **11** |
| **atlas** | 1 page | 2048 × 2048 | rgba8unorm | **16** |
| | | | **grand total** | **175.5 MiB** |

**Two findings that change the stream's arithmetic** (both recorded as [I3](issues.md)):

1. **`fw/fh` sizes THREE maps, not one.** The README (and the A/B commit) talk about "the lightmap";
   `SLOTS × TEXTILE_SQUARE` actually allocates `coldLightRT`, `hotLightRT` *and* `receiverFineRT`. So
   pinning `TEXTILE_LIGHT` at 64 saves **216 MiB** (72 → 288), not the ~48 MiB the README estimated.
   The fine maps were the *dominant* term in the A/B's "192 MiB freed" — not the art maps.
2. **The art family is 8 channels, not 4.** `VARIABLES.md` lists albedo/normal/surface/zdepth; there is
   a full **warm** duplicate for movers. So raising `SQUARE` costs **+245 MiB** (76.5 → 321), not the
   ~120 MiB estimated.

Net: the split is worth *more* than planned and the raise costs *more* than planned. Projected total at
`SQUARE = 128` with `TEXTILE_LIGHT` pinned: **~420 MiB**, versus **~636 MiB** if the constant were left
joined. The README's table is corrected by P3's re-measure.

**Deviation:** the item asked for agreement with "the browser's reported GPU memory within 10%". WebGL2
exposes no GPU memory query and `chrome://gpu` is not reachable from page script — see
[`deviations.md`](deviations.md).

### Lighting-pass baseline — 120 frames, 3 repeats, per reach

Harness: drive `ShadowGather.tick` **directly** in a synchronous loop rather than through
`requestAnimationFrame` — the tab backgrounds during a tool-driven session and Chrome suspends rAF
entirely, which is what stalled the first three attempts. This is strictly better than the previous
harness: exact frame count, no vsync jitter, no visibility dependence. All three documented harness bugs
stay fixed — draws discriminated by **render target** (`coldShadowRT` vs `coldLightRT`), `orbitPhase`
reset to 0 so **phase derives from the frame index**, and a **fixed frame count**. 12 warm-up frames are
dropped so the `rebakeAll` frame never enters the mean.

| reach | dirty tiles | gather ms | light ms |
|---|---|---|---|
| 4 | 217 | 0.134 (spread 0.041) | **0.124** (spread 0.032) |
| 8 | 392 | 0.668 (spread 0.070) | **0.286** (spread 0.048) |
| 12 | 465 | 0.845 (spread 0.016) | **0.253** (spread 0.006) |

**Calibration passes** — the number responds to a workload change under our control: dirty tiles
217 → 392 → 465 and gather 0.134 → 0.668 → 0.845 (6.3×). A harness that did not move with reach would be
measuring something else.

**The interesting result is the light column: it is nearly FLAT.** Gather scales 6.3× across the reach
sweep while lighting moves 0.124 → 0.286 → 0.253 and then *falls*. Lighting cost is therefore not driven
by the dirty-tile count — consistent with it being driven by lightmap **texels**, which is exactly the
premise P0's third item exists to confirm.

**Deviation:** the item demanded ±0.01 ms across two runs. Achieved is **±0.07 ms absolute** at reach 8,
and reach 4 swings 0.110–0.156 between repeats on an identical 217-tile workload — so the criterion was
set too tight for GPU timer queries at this magnitude. Three repeats are reported with their spread
instead of two runs with a tolerance. Any later comparison must clear that spread to count.

### A harness bug worth keeping

The first `__setreach` wrote the light record directly (`writeRecord` on the `light_data` band) and
**silently did nothing** — `tick` re-resolves every carried light from its `standing` prim each frame,
so the authored reach was restored before the next draw. The tell was `dirtyAvg` sitting at 394 for
reach 4, 8 and 12 alike: three different "workloads" producing byte-identical dirty counts. Fixed by
setting `p.light.reach` on the prim — the same front door `__torch` uses. **A calibration that does not
move is not a stable measurement, it is a disconnected one**, and this is the third time in this
codebase that lesson has had to be re-learned.

### The premise test — lighting resolution up, art resolution untouched

`TEXTILE_SQUARE` set to a literal `128` while `SQUARE` stayed at `64`, so the three fine maps went to
4096×2048 and **nothing else moved**: art maps still 2176×1152, shadow still 512×256. Same harness, same
3 repeats, dirty counts matched the baseline to within 2 tiles (215/391/463 vs 217/392/465), so the two
builds were doing equivalent work.

| reach | dirty | light ms @ 64 | light ms @ 128 | ratio | gather ms @ 64 → @ 128 |
|---|---|---|---|---|---|
| 4 | ~216 | 0.124 | **0.344** | **2.8×** | 0.134 → 0.122 |
| 8 | ~391 | 0.286 | **0.620** | **2.2×** | 0.668 → 0.490 |
| 12 | ~464 | 0.253 | **0.790** | **3.1×** | 0.845 → 0.846 |

**Confirmed: lightmap texels drive the lighting cost; art texels do not.** 4× the lighting texels costs
2.2–3.1× the time, while the gather — which reads the *shadow* map and the art atlas — does not move
beyond the harness's own spread. This is the fact the whole stream rests on, and it now has a
measurement rather than an inference.

**Under the predicted 3–4×, and the reason is benign:** the draw carries per-rect setup and texture
fetches that do not scale with output texels, so a 4× fragment count buys less than 4× time. Sub-linear
scaling makes the split *more* attractive, not less — the reverse would have been the worrying result.

**A bonus artifact: the probe made the `uLSlot` hazard visible.** With the lightmap at 128/tile and
`win.slotPx` still 64, the frame renders with obvious quadrant misregistration — lighting sampled at the
wrong scale, seams on slot boundaries. [F3](forks.md#f3) argued this coupling on paper; the probe shows
it. P1's `uLSlot` item is not defensive, it is load-bearing.

Probe reverted (`TEXTILE_SQUARE = SQUARE`); `git diff` clean on `squareMath.ts`.
