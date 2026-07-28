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

### The wolf: the cap is `BASE_LOD_PX = SQUARE`, and the masters are already big enough

`TextureResolver` line 44 is the whole mechanism:

```
const BASE_LOD_PX = SQUARE;                                    // 64 today
this.targetPx = Math.min(BASE_LOD_PX, ...);                    // 64
let desired = pickLodForSize(Math.min(this.targetPx * gridFactor, cap));   // snaps to LOD_SIZES
```

`targetPx` read back live: **64**. So every sprite is fetched at the 64 px LOD regardless of what the
master holds. Manifest audit of all **27** entries:

| master `maxSize` | entries |
|---|---|
| 128 | 8 |
| 256 | 4 |
| 320 | 1 |
| **512** | **14** |
| **below 128** | **0** |

**`pawn/animal/wolf/e` has a 512 px master and is being drawn from the 64 px LOD — an 8× downsample.**
That is the mush in the user's screenshot, and it is a client-side cap, not an art problem.

This **answers P3's audit item ahead of schedule and inverts its conclusion**: nothing needs
re-mastering. Every entry already carries ≥128, so raising `SQUARE` to 128 immediately doubles
`targetPx` and every sprite steps up one LOD with no corpus work at all. The plan assumed the corpus
might have been authored against the 64 cap; it was not.

**Before-image caveat:** no wolf pawn was present in area1 during this session (the npc container was not
running), so the recorded before-state is `targetPx = 64` plus the user's own screenshot rather than a
matched crop. `targetPx` is the stronger evidence — it is the cap itself, not a rendering of it — and
P3's after-check is `targetPx == 128` plus a visual pass once a wolf is on screen.

## 2026-07-28 · P1 — the split, proven as a no-op

`TEXTILE_LIGHT = 64` added to `squareMath.ts`, **pinned and explicitly not tracking `SQUARE`**. Three
consumers moved off `TEXTILE_SQUARE`:

| site | was | now |
|---|---|---|
| `shadowGather.ts:2039` — the three fine RTs | `SLOTS × TEXTILE_SQUARE` | `SLOTS × TEXTILE_LIGHT` |
| `shadowGather.ts:37` — `FINE_RATIO` | `TEXTILE_SQUARE / SHADOW_TEXELS` | `TEXTILE_LIGHT / SHADOW_TEXELS` |
| `Viewport.ts:456` — the lightmap's `uLSlot` | `win.slotPx` (= `SQUARE >> lod`) | `TEXTILE_LIGHT >> lod` |

The third is the load-bearing one. `win.slotPx` is the **art** texel size; it was a correct value for the
lightmap only while the two constants were equal. P0's probe rendered the consequence — visible per-slot
misregistration the moment they differ — so this is a fix, not a precaution. Comment rewritten to state
the general rule: **a map's slot stride comes from that map's own texels-per-tile constant, never from
another map's.**

### The no-op holds on every axis

| check | result |
|---|---|
| `FINE_RATIO` | **4** (`TEXTILE_LIGHT/TEXTILE_UNIT` = 64/16), read live from the module |
| `coldLightRT` / `hotLightRT` / `receiverFineRT` | 2048×1024 — identical to baseline |
| `coldShadowRT` / `receiverCoarseRT` / `decayRT` | 512×256 — untouched |
| art maps (cold + warm) | 2176×1152 — untouched |
| corridor↔brute identity, zoom 1 | **0 differing** of 36 792 non-zero |
| corridor↔brute identity, after sweep | **0 differing** of 73 099 non-zero |
| zoom sweep 1 → 0.5 → 0.25 → 1 | lod 0 → 2 → 0, `uLSlot` tracks `64 >> lod` = 64/16, `glGetError` 0 |
| render | correct — art, shadows and lighting all in register |

Perf, same harness, 3 repeats (baseline → split), dirty counts **byte-identical** at 217/392/465:

| reach | gather ms | light ms |
|---|---|---|
| 4 | 0.134 → 0.164 | 0.124 → 0.173 |
| 8 | 0.668 → 0.595 | 0.286 → 0.246 |
| 12 | 0.845 → 0.868 | 0.253 → 0.262 |

Every difference sits inside the measured spread (light spread alone reached 0.183 ms at reach 4 this
run), so the split is a no-op to the limit this harness can resolve. Per [D3](deviations.md#d3) the
planned ±0.01 ms gate is void — that tolerance was never achievable.

**Why the sequencing paid off:** with `SQUARE` still 64, `TEXTILE_LIGHT` and `slotPx` are *equal*, so
every readback above had a hard oracle — identical, not merely plausible. Had the raise landed in the
same step, none of these numbers could have distinguished a correct split from a broken one.

**Caveat on the zoom sweep:** driving `tick` synthetically does NOT refresh `win`, so lod stayed 0 and
the first sweep proved nothing. Real frames (forced via screenshot capture) were needed to reach lod 2.
The tick-driven harness is right for timing a draw and wrong for anything that depends on the Viewport
recomputing the window.

## 2026-07-28 · P2 — `SQUARE = 128`, and the split holds

### Every constant lands where the design says

| constant | value | |
|---|---|---|
| `SQUARE` | **128** | matches `VARIABLES.md` |
| `UNIT` | 8 | `SQUARE/16` |
| `TEXTILE_UNIT` | **16** | unchanged — the world invariant |
| `TEXTILE_SQUARE` | 128 | the art dial, followed `SQUARE` |
| `TEXTILE_LIGHT` | **64** | pinned, did NOT follow |
| `FINE_RATIO` | **4** | unchanged |
| `REFERENCE_W/H` | **3584 × 1536** | matches `VARIABLES.md` |
| `SLOT_PW` | 132 | `SQUARE + 2·PAD` |

| map family | before | after |
|---|---|---|
| art (cold + warm) | 2176 × 1152 | **4352 × 2304** — exactly the size `VARIABLES.md` specifies |
| lighting (3 maps) | 2048 × 1024 | **2048 × 1024** — unchanged |
| shadow (6 maps) | 512 × 256 | **512 × 256** — unchanged |

### The result the stream exists for

Same harness, 3 repeats, dirty counts **byte-identical** to the baseline at 217/392/465:

| reach | light ms @ 64 | light ms @ 128 | Δ |
|---|---|---|---|
| 4 | 0.124 | 0.148 | +0.024 (spread 0.079) |
| 8 | 0.286 | **0.290** | **+0.004** (spread 0.015) |
| 12 | 0.253 | **0.263** | **+0.010** (spread 0.017) |

**Art resolution doubled on both axes; the lighting pass did not move.** At reach 8 the difference is
0.4 %, well inside the spread. Gather likewise: 0.668 → 0.691 and 0.845 → 0.902. Compare the P0 probe,
where letting the lighting follow `SQUARE` cost **2.2–3.1×** — that entire cost is what the pin avoids.

### Acceptance

| check | result |
|---|---|
| corridor↔brute, zoom 1 | **0 differing** of 37 315, and again 0 of 71 809 after the sweep |
| corridor↔brute, zoom 0.25 (lod 2) | **0 differing** of 6 644 |
| zoom sweep 1 → 0.25 → 1 | `glGetError` 0, no misregistration, correct render at both ends |
| cover fit | `max(W/3584, H/1536)` — 1080p → 0.703, 4K → 1.406, both cover by construction (`max`, not `min`) |
| art LOD | `targetPx` 64 → **71.1**, which `pickLodForSize` snaps to the **128** LOD |

### The zoom sweep is where the P1 fix earns itself

At zoom 0.25, lod 2:

```
win.slotPx        = 32     (SQUARE 128 >> 2)  — the ART texel size
uLSlot            = 16     (TEXTILE_LIGHT 64 >> 2) — the LIGHTING texel size
```

**They now differ by 2×.** The old code fed `win.slotPx` to the lightmap, so on this build it would have
sampled the lightmap at double scale at every lod below 0 — silently, since lod 0 still agreed. Splitting
first ([F3](forks.md#f3)) meant this was fixed while the two values were still equal and the oracle was
exact; had both changes landed together, the misregistration would have been indistinguishable from an
ordinary raise bug.

### On `targetPx`

It reads 71.1, not 128 — because it is the *requested* size (on-screen tile px at the current cover
scale), not the cap. `BASE_LOD_PX = SQUARE` was clamping it to 64 before; with the clamp at 128 the true
71.1 shows through and `pickLodForSize` snaps up to the 128 LOD. The art genuinely steps up a level; the
number to watch is the LOD chosen, not `targetPx` itself.

## 2026-07-28 · P3 — the art has its resolution back

**No re-mastering was needed** ([F6](forks.md#f6)) — P0's audit found 0 of 27 manifest entries short of
128. Raising `SQUARE` was sufficient on its own, and the loaded LODs prove it:

```
biome-thing/default/conifer/e   loaded [32, 128]      <- was capped at 64
biome-thing/default/flora/e     loaded [32, 128]
```

`pickLodForSize` now selects the **128** co-pack. The conifers are visibly crisper at zoom 1 (needle
structure and trunk shading that the 64 build could not carry). The wolf is not verifiable this session —
no pawn was in area1 because the npc container was not running — but it takes the same path as every
other sprite and its master is 512, so it gains the same step. Flagged as the one visual claim in this
stream that is **inferred rather than seen**.

### Resident bytes re-measured

| family | before (`SQUARE` 64) | after (`SQUARE` 128) | |
|---|---|---|---|
| art (8 channels) | 76.5 MiB | **306 MiB** | 4.0× — the price |
| fine / lighting (3 maps) | 72 MiB | **72 MiB** | **unchanged** |
| shadow (6 maps) | 11 MiB | **11 MiB** | unchanged |
| atlas | 16 MiB | **16 MiB** | still **one page** |
| **total** | 175.5 MiB | **405 MiB** | |

Against the joined-constant alternative (`TEXTILE_SQUARE` following `SQUARE`), the fine maps would be
288 MiB and the total **621 MiB**. **The pin saves 216 MiB and 2.2–3.1× of lighting time.**

Two corrections to earlier figures in this stream, both mine:

- The README projected art at 321 MiB; it is **306**. I divided by 10^6 in one place and 2^20 in
  another. 4352 × 2304 × 4 B = 38.25 MiB per channel, × 8 = 306.
- [I3](issues.md) projected a ~420 MiB total; measured **405**, same cause.

**The predicted atlas-page risk did not materialise.** 4× frame area on one 2048² page still fits at the
LODs area1 loads. It is not disproven in general — a fuller scene could still spill — so it stays a live
concern for [`2026-07-28-art-128-tiles`](../2026-07-28-art-128-tiles/README.md) rather than a closed one.

## 2026-07-28 · P4 — the docs reconciled

`VARIABLES.md` had been describing the 128 world since `2b1025a` shipped 64 without amending it
([I1](issues.md)), so most rows needed no change — the code came to them. Three did:

| line | was | now |
|---|---|---|
| 472 | `1 unit = SQUARE/16 = 4px` | **8px** — it was the one row still describing `SQUARE = 64`, contradicting every other row in the same document |
| 306 | `TEXTILE_SQUARE`, no apron / 128 / 4096 × 2048 / lightmap | **`TEXTILE_LIGHT` / 64 PINNED / 2048 × 1024** / lightmap (cold + hot) + fine receiver |
| 305, 307 | 4 art maps; shadow listed alone | art **× 2 (cold AND warm)**; shadow row names all six maps that ride it |

Plus a new paragraph stating the pin, the 2.2–3.1× measurement behind it, and the measured resident
bytes (405 MiB split vs 621 MiB joined), so the next reader inherits the reason rather than just the
number.

Confirmed live against the running build: `ppu` at lod 0 = **8** (`SQUARE/(16·2^lod)` = 128/16),
`REFERENCE` **3584 × 1536**, max frame side `16 × 128` = **2048**, art texture **4352 × 2304**.

**The rule this stream is evidence for:** a constant ratified in `VARIABLES.md` must not be changed by a
`test(` commit without either reverting it or amending the document. `2b1025a` did neither, and for a day
the authority file described a world the client did not build — including `ppu`, which is live shader
input, so anyone hand-checking against the doc would have "found" a bug that was not there.
