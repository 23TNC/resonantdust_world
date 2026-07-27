# Issues — problems hit, candidates, what we chose

## I13 — Shadow map raised to the lightmap's resolution: 3.2× GPU, 4× VRAM, no more upsample {#i13}
_2026-07-27 · behind tag `checkpoint-lighting-2026-07-27`_

`SHADOW_TEXELS` changed from `TEXTILE_UNIT` (16/tile) to `TEXTILE_SQUARE` (= `SQUARE`, 64/tile), so
**`FINE_RATIO` becomes 1** — a light texel and its shadow texel are now the same texel and the fine→coarse
upsample disappears entirely.

| | before | **after** |
|---|---|---|
| shadow RT | 512 × 256 (2 MiB/att) | **2048 × 1024** (32 MiB/att) |
| lightmap RT | 2048 × 1024 | *unchanged* |
| **shadow VRAM** (4 RTs × 2 att) | 16 MiB | **256 MiB** |
| **lighting stack VRAM** | **80 MiB** | **320 MiB** |
| lighting pass | 0.444 ms | **0.399 ms** |
| **gather** | 2.644 ms | **9.444 ms** |
| **GPU total** | **3.09 ms** | **9.84 ms** |

### The gather did NOT scale with fragment count, and that is the interesting part
16× more fragments produced only **3.6× more time** — a **4.4× better per-fragment rate**. The most likely
cause is cache locality: at 16× density, adjacent fragments walk nearly identical corridors, so their bucket
fetches and caster-record loads land in cache that the coarse map missed. It is consistent with
[I11](#i11) finding the walk ALU-bound rather than fetch-bound — the extra fragments mostly re-run arithmetic
over data already resident.

**So the per-texel walk is much cheaper than the coarse map's cost-per-texel implied.** That is worth keeping
in mind for [F4](forks.md#f4): its ~8× reduction in caster *tests* may not translate to an ~8× time saving,
because the tests it removes are the cheap cached ones, not the expensive cold ones.

### What it buys
The blocky shadow edge is gone by construction — there is no upsample left to be blocky. This is the
alternative to the deleted refine: rather than re-walking per fine texel to sharpen a coarse edge, the shadow
is simply computed at the fine resolution in the first place. **3.2× GPU and 4× VRAM for that**, versus the
refine's 7.7× GPU for the same visual outcome — so it is strictly the better of the two ways to get a sharp
edge, and still much more expensive than accepting the coarse one.

Lighting also got marginally cheaper (0.444 → 0.399 ms) since `FINE = 1` removes the upsample divide.

**Not a recommendation either way — this is the measured cost of the option.** Revert is the one constant
`SHADOW_TEXELS`, or `git checkout checkpoint-lighting-2026-07-27`.

## I12 — `SQUARE` 128 → 64: lighting 3.3× cheaper, 192 MiB freed, art visibly softer {#i12}
_2026-07-27 · A/B, same fixture (one moving reach-16 light, 6-tile orbit)_

`SQUARE` is the only dial on the fine lightmap's size — `TEXTILE_SQUARE = SQUARE`, and the lightmap is
`SLOTS · TEXTILE_SQUARE`. `TEXTILE_UNIT` is fixed at 16/tile, so **`shadow-cold` is unaffected**.

| | SQUARE 128 | **SQUARE 64** |
|---|---|---|
| lightmap RT | 4096 × 2048 · **128 MiB** ea | 2048 × 1024 · **32 MiB** ea |
| shadow RT | 512 × 256 · 2 MiB ea | *unchanged* |
| **lighting stack VRAM** | **272 MiB** | **80 MiB** (−192) |
| **lighting pass** | 1.462 ms | **0.444 ms** (3.3×) |
| gather | 2.378 ms | 2.644 ms (unchanged code; orbit-phase noise) |
| **GPU total** | 3.84 ms | **3.09 ms** |

### The oversample flips sign
This is the [I4](#i4) oversample resolved from the other end:

| | lightmap texels | vs 3.00 M canvas px |
|---|---|---|
| SQUARE 128 | 8.39 M | **2.79× over** |
| SQUARE 64 | 2.10 M | **0.70× — slightly UNDER** |

So 128 was ~2.8× more lighting resolution than the display can show, and 64 is a little less. The ideal sits
between, but there is no pow2 in between, and lighting is low-frequency enough that under-sampling it costs
less than under-sampling art.

### The real cost is ART, not lighting
On-screen framing is **identical** — `coverScale` compensates (`REFERENCE` halves, scale doubles), so the same
28 × 12 tiles fill the screen and a tile still displays at ~97.6 px. What changes is where that tile's pixels
come from:

- SQUARE 128: art authored at ≤128 px shown at 97.6 px → **0.76× downscale (crisp)**
- SQUARE 64: art authored at ≤64 px shown at 97.6 px → **1.53× upscale (visibly soft)**

Confirmed at zoom 1: sprites are noticeably softer than the SQUARE-128 shots. **Shadow blockiness is
unchanged** — the shadow map is 16 texels/tile either way and a tile occupies the same screen size, so it is
6.1 screen px per shadow texel in both.

**So this is not a lighting-quality trade, it is an ART-resolution trade** — and the art cap drops from 128 px
to 64 px, which is a content decision, not a renderer one. Left at 64 pending that call; reverting is the one
constant in `squareMath.ts`.

## I11 — Inside `walkShadow`: it is NOT fetch-bound. `casterOne` is 77 %. {#i11}
_2026-07-27 · `uWProfile` staircase, one moving reach-16 light, zoom 0.5_

| level | includes | ms | **substep** | share |
|---|---|---|---|---|
| 1 | DDA setup only | 0.065 | 0.065 | 2 % |
| 2 | + tile traversal (no bucket fetch) | 0.078 | +0.013 | 1 % |
| 3 | + **bucket fetches** (3 per tile) | 0.251 | **+0.173** | **6 %** |
| 4 | + slot unpack (`tileSlot` × 8) | 0.620 | **+0.369** | **14 %** |
| **0** | **+ `casterOne`** | **2.679** | **+2.059** | **77 %** |

### The walk is not fetch-bound, and I had asserted that it was
**Texture fetches are 0.173 ms — 6 % of the walk.** The cost is `casterOne`: the per-caster projection,
point-in-quad test and silhouette sample. Even the *slot unpack* (0.369 ms) costs **2× more than the fetches**.

**This refutes a claim I made while costing [F6](forks.md#f6).** I argued that evaluating both receivers per
caster would be "much closer to free than to 2×" because *"the walk is fetch-dominated (3 fetches/step × 8
slots), so sharing the fetches and doubling only the per-caster arithmetic"* is cheap. **The arithmetic IS the
cost.** Doubling the per-caster work doubles 77 % of the walk. F6's split-shadow gather is therefore
**substantially more expensive than I estimated** — closer to +1.5 ms than to the "well under 0.3–0.5 ms" I
wrote. That estimate was reasoning from an assumed bottleneck instead of a measured one, which is the same
error as the three cost models before it.

### What it means for [F4](forks.md#f4)
F4 (bucket casters by where their **shadow** lands) still wins, but for a different reason than the one I gave.
It is not about fetching less — it is about **testing fewer casters**. At this world's ~0.65 casters/tile, a
16-tile corridor with 3-wide dilation visits ~48 tiles and tests **~31 real casters per texel**; F4 reduces
that to the ~4 whose shadow actually reaches the texel. That is an **~8× cut on the term that is 77 % of the
walk**, which is 2.06 of the 3.8 ms GPU frame.

### Current full attribution, one moving reach-16 light, zoom 0.5

| item | ms | share of ~3.8 ms GPU |
|---|---|---|
| `casterOne` inside the walk | **2.06** | **54 %** |
| `receiverAt` (lighting pass) | 0.98 | 26 % |
| slot unpack + bucket fetches + DDA | 0.62 | 16 % |
| everything else | ~0.15 | 4 % |

## I10 — Gather substep profile: `walkShadow` is 97 % of it {#i10}
_2026-07-27 · same `uGProfile` staircase device as [I9](#i9), one moving reach-16 light, zoom 0.5_

| level | includes | ms | **substep** |
|---|---|---|---|
| 1 | dirty gate + window mapping + `P` | 0.015 | 0.015 |
| 2 | + `receiverAt` | 0.053 | +0.038 |
| 3 | + the `allBillboard` corner test (3× `receiverCover`) | 0.057 | +0.004 |
| 4 | + presence fetch and the light loop, **no** `walkShadow` | 0.069 | +0.012 |
| **0** | **+ `walkShadow`** | **2.341** | **+2.272** |

**The gather is essentially pure corridor walk: 2.272 of 2.341 ms — 97 %.** Everything else in it — the
window mapping, the receiver mask, the corner test, the presence fetch, the whole 16-slot light loop and the
MRT pack — costs **0.069 ms combined**.

Worth noting the contrast with [I9](#i9): `receiverAt` costs **0.038 ms here and 0.978 ms in the lighting
pass**, the same function called at 131 k texels versus 8.39 M — the 64× resolution gap showing up exactly
where it should.

### Where the remaining frame goes
For one moving reach-16 light at zoom 0.5, GPU ≈ **3.8 ms**:

| item | ms | owner |
|---|---|---|
| `walkShadow` (gather) | **2.27** | [F4](forks.md#f4) — bucket casters by where their SHADOW lands |
| `receiverAt` (lighting) | **0.98** | [F7](forks.md#f7) — the prim pass |
| everything else | 0.55 | — |

**Those two items are 86 % of what is left.** Both are the same shape — a per-texel search for something the
geometry already knows — and both were identified before this profile rather than after, which is the first
time in this stream that the measurement has confirmed the plan instead of overturning it.

## I9 — Lighting-pass substep profile: the edge refine is 85 % of it {#i9}
_2026-07-27 · one moving reach-16 light, zoom 0.5, `uProfile` staircase + GPU timer_

The lighting bake is a **single draw**, so substeps cannot be timed directly. Added a `uProfile` uniform that
cuts `LIGHT_FRAG` short at named boundaries; each level includes every level below it, so the **difference**
prices one substep.

| level | includes | ms | **substep cost** |
|---|---|---|---|
| 1 | dirty gate + window mapping + `P` | 0.161 | 0.161 |
| 2 | + `receiverAt` | 1.270 | **+1.109** |
| 3 | + `billboardNormal` + `worldNormal` | 1.298 | +0.028 |
| 4 | + `accumulateLights`, **no** shadow fetch | 1.571 | +0.273 |
| 5 | + coarse shadow lookup | 1.589 | +0.018 |
| **0** | **+ edge refine** | **10.875** | **+9.286** |

**The edge refine is 9.29 ms of a 10.88 ms pass — 85 %.** Everything else in the entire lighting bake,
including the light loop, the normal sampling and the shadow lookup, costs **1.59 ms combined**.

Second place is `receiverAt` at **1.11 ms** (10 %) — the per-fine-texel receiver mask. Third is the light
accumulation loop itself at 0.27 ms. The coarse shadow fetch is **0.018 ms**, i.e. free.

**Why the refine is so expensive:** it re-runs the full `walkShadow` corridor at the **fine lightmap
resolution** — 8.39 M texels versus the shadow map's 131 k, a **64×** resolution multiplier — for every texel
whose coarse coverage is a partial `(0,1)` edge value. The gate works (interior 0/1 texels skip it), but shadow
edges are numerous enough that 64× resolution swamps the saving.

**Consequence.** The whole optimisation target for moving lights is one feature: the fine shadow-edge refine.
The shadow *gather* (1.6 ms), the light loop (0.27 ms) and the coarse shadow lookup (0.02 ms) are all noise
beside it. Options, in the order they should be tried: tighten the gate (only refine where the edge is
actually visible at display resolution), refine at a lower multiplier than 64×, cache the refined edge instead
of recomputing it per frame, or drop the refine and accept the coarse edge — it is a toggle, so its visual
value can be judged directly against 3.5× frame time.

## I8 — The cost is the FINE LIGHTMAP BAKE, not the shadow gather. Everything above mis-attributed it. {#i8}
_2026-07-27 · per-pass GPU profile. **This supersedes the attribution in [I4](#i4), [I6](#i6) and
[`plan-4096.md`](plan-4096.md), all of which aimed at the wrong pass.**_

### Per-pass profile — one moving reach-16 light, zoom 0.5

| stage | where | ms |
|---|---|---|
| **lighting** (fine lightmap bake, 4096 × 2048) | GPU | **9.72** |
| gather (shadow-cold, 512 × 256) | GPU | 1.63 |
| `buildCasters` | CPU | 2.1 |
| `buildPresence` / `buildDirty` / `flush` | CPU | 0.2 / 0.1 / 0.1 |

**The shadow gather is 1.6 ms. The lightmap bake is 9.7 ms — 86 % of GPU time.**

### The hot spot inside it is the edge refine, and it is a toggle

`LIGHT_FRAG` re-runs `walkShadow` **per fine texel** to sharpen the shadow edge
([2026-07-24-shadow-edge-refine](../2026-07-24-shadow-edge-refine/README.md), fused into the lighting pass).
The fine lightmap has **8.39 M texels against the shadow map's 131 k — 64×**. Measured with `uEdgeRefine`:

| | lighting | gather | total GPU |
|---|---|---|---|
| edgeRefine **ON** | 9.72 | 1.63 | **11.36** |
| edgeRefine **OFF** | **1.42** | 1.86 | **3.28** |

**The refine costs 8.3 ms of 11.4 — 73 % of the whole GPU frame, for one light. Disabling it is 3.5×.**

### This also explains the zoom inversion, exactly as the user predicted

| zoom | map tiles | dirty | dirty % | lighting | gather | GPU total |
|---|---|---|---|---|---|---|
| 1 | 512 | 496 | **97 %** | **15.69** | 1.82 | **17.51** |
| 0.5 | 2 048 | 1 050 | 51 % | 10.82 | 1.82 | 12.64 |
| 0.25 | 8 192 | 3 340 | 41 % | 5.77 | 1.18 | **6.95** |

Zooming **in** costs **2.5×** more while covering 16× less world and dirtying 6.7× fewer tiles. **The gather is
flat (~1.8 ms) at every zoom** — it never varied. All of the zoom dependence lives in the lightmap bake, whose
cost is `dirty fraction × 8.39 M fixed texels`, and the dirty fraction is worst zoomed in because a reach-16
light covers ~97 % of a 512-tile map and only 41 % of an 8 192-tile one.

### What this invalidates

- **"≥81 % of the frame is the walk" ([I6](#i6)) was right about *shadows* and wrong about *where*.** Turning
  off `castShadows` disables the walk in *both* passes; I attributed the saving to the gather's corridor. It is
  overwhelmingly the fine re-walk in the lighting pass, at 64× the resolution.
- **[`plan-4096.md`](plan-4096.md) P1 — the per-texel walk budget — targets the gather, which is 1.6 ms.**
  Making the gather free saves ~14 % of the frame. The plan's central lever was aimed at the wrong pass.
- **The 2.79× oversample ([I4](#i4)) is not noise; it is now the single biggest structural lever.** I dismissed
  it twice — first promoting it wrongly, then demoting it wrongly. It multiplies the 8.39 M-texel bake that
  dominates everything, so sizing the lightmap to the canvas cuts the dominant pass directly.
- **`buildCasters` at 2.1 ms CPU is real** and would bind well before 4096 lights.

**Method note:** every earlier conclusion in this stream rested on wall-clock frame time, which is pinned at
the 8.33 ms vsync floor and therefore measures nothing until the frame is already blown. A per-pass GPU timer
found in one pass what four rounds of wall-clock A/B could not, and reversed the ranking of every lever. The
instrument was the bottleneck, not the analysis.

## I7 — Why the current version walks: the caster data lost its light association {#i7}
_2026-07-26 · traced through the design record_

**The 2026-07-21 design does not walk.** From
[`shadow-bitfield/README.md`](../2026-07-21-shadow-bitfield/README.md), the user's own design, point 4:

> _"For a dirty pixel, loop the tile's present lights; **per light walk its `caster_count`-bounded casters**,
> box-cull, test point-in-silhouette, OR its bit into a register, write the `u128` once."_

"Walk its casters" there means **iterate that light's own caster list** — a per-**light** LUT, `≤ 256 casters/
light`, living in `light_data` (128 × 33: row 0 lights, rows 1–32 the LUT). No spatial marching anywhere.
[F2](../2026-07-21-shadow-bitfield/forks.md) confirms the intended bound: _"per-fragment × per-reaching-light
× per-caster — bounded by the box cull, LUT-bounded casters."_

**The current design does walk, and says so.** [`VARIABLES.md`](../../VARIABLES.md) — authoritative — now
specifies `billboard_presence` as a per-**tile** bucket: _"A caster is bucketed into every tile its tilted
card's ground extent spans… **The gather's reach-walk reads these**"_, with the per-texel bound stated as
_"≤ 8 lights (presence) × **reach-walk of bucketed tiles** × ≤ 8 casters/tile."_

**So this is not code deviating from design.** The design changed — the per-light caster LUT was replaced by
per-tile caster buckets, around `presence-in-data` (2026-07-23), which folded "light-presence + caster-buckets"
into the unified data texture. VARIABLES records the result as current truth and the code matches it.

### Why the walk is there, mechanically
A per-tile bucket keyed by the caster's **own body** is cheap to maintain (one entry per caster per tile it
covers, shared by every light) — but it **throws away the light↔caster association** the per-light LUT had.
Once that association is gone, a texel cannot know which casters shadow it. The only way back is to search the
space between the texel and its light. **The walk is the price of not storing the association.**

### Why going back to the 2026-07-21 LUT would be worse, not better
Worth stating, because "restore the original design" is the obvious move and the numbers say don't. A
reach-16 light covers ~1 024 tiles; at the ~0.5 casters/tile this world runs, its LUT would hold **~500
casters**, and every texel in its disc would test all 500 with only a box cull to prune. The walk visits ~16–32
corridor tiles × ≤ 8 slots and, crucially, **prunes spatially** — most slots are empty and exit immediately.
**The walk is not a mistake; it is the acceleration structure that replaced an unbounded per-light list.** That
is very likely why the design changed in the first place.

### What actually beats both
Bucket casters by **the tiles their SHADOW lands on, per light** — not by the tiles their body occupies. That
keeps the spatial pruning (a texel reads only its own tile) *and* restores the light association (the bucket is
per light, so entries are exactly the casters that shadow this tile from that light). Per texel the cost
collapses to "the casters that actually shadow me", with no march. Maintenance is `Σ over lights of (casters in
reach × tiles their shadow covers)` — rebuilt on the same `markLightDirty` / `markPrimDirty` events that
already exist.

It is strictly more state than today's shared body buckets, and that is the real trade to weigh: today's
buckets are shared across all lights and cost O(casters); shadow buckets are per light and cost
O(lights × shadow area). Not yet costed — see [F4](forks.md#f4).

## I6 — Today's walk vs this morning's: 1.54×, bit-identical output, same complexity class {#i6}
_2026-07-26 · measured A/B_

### The two methods, from git (not from memory)
Both are the **same algorithm class** — a *gather*: relight dirty tiles → per texel → per light present in
that tile (≤ **16**, `PRES_SLOTS`) → walk toward the light → test the casters bucketed in each visited tile
(≤ **8**, `BILLBOARD_SLOTS`).

| | this morning (pre-`5bd274f`) | now |
|---|---|---|
| walk | **point-sample** the light→Q segment at ≤1-tile spacing, ≤48 samples | **exact supercover DDA** (Amanatides-Woo), ≤64 tiles |
| pad | **5-tile cross** per sample (centre ±x ±y) | **3-tile perpendicular-only** dilation |
| fetches/step | 5 | 3 |
| casters/fetch | 8 | 8 |

The pad exists because a caster is bucketed by its *tight-bbox ground cover* while `casterCover` tests a wider
projected extent, so a caster in tile T can occlude a ray through T±1. I30's insight was that consecutive walk
tiles already supply each other's ±1 *along* the direction of travel, so only the **perpendicular** neighbours
are load-bearing.

### The measurement
A `DILATE` constant now switches the shader between the two, so this is an A/B and not a commit message.
Identical scene (1 326 standing prims both runs), identical lights (32, chosen deterministically as the
nearest prims to a fixed world point — fingerprint `8332, 8340, 8341, 8342, 8347` in both), reach 16, zoom 0.5,
all orbiting:

| dilation | ms | fps | static shadow hash |
|---|---|---|---|
| **5** — this morning's cross | 229.18 | 4 | `276261732` / 172 019 nz |
| **3** — today's perpendicular | **149.01** | 7 | `276261732` / 172 019 nz |

**1.54× faster for a bit-identical shadow map.** Same hash, same non-zero population — the narrower dilation
loses nothing. Today's version is a strict improvement and I30's claim holds up.

### But it did not change what matters
Both are `O(lit-texels × walk-length × casters-per-tile)`. The constant fell by a third; the exponent did not
move. Ceiling for 60 fps at reach 16 went from **~8 moving lights to ~13**. That is the entire difference.

And the cost is *all* shadow — 35 moving reach-16 lights measured **43.23 ms with casting on vs 8.33 ms (the
vsync floor) with casting off**, i.e. **≥81 % of the frame is the walk**, with the full 4096 × 2048 lightmap
bake, falloff, N·L and accumulation being effectively free. So:

- Further constant-factor work inside the gather (fewer fetches, better culling) buys tens of percent.
- The **2.79× oversample** ([I4](#i4)) is a flat win but applies to the cheap half — I previously called it
  "the one worth taking next", which was **wrong**: shadows outweigh it ~5:1.
- Reaching hundreds of moving lights needs a **different complexity class**, not a faster walk.

### The complexity-class candidate
The walk exists to answer *"which casters lie between this texel and its light"* — and it re-answers it from
scratch for every texel. The alternative is to **bucket casters by the tiles their SHADOW lands on** rather
than by the tiles their body occupies. Then a fragment loops **only its own tile's list** and the walk
disappears entirely: cost per texel drops from `walk × 3 × 8` (up to 1 536 caster tests) to one list of ≤64.
That is a CPU/geometry-side scatter feeding the same per-tile bucket the gather already reads, so the shading
math, the max-accumulate invariant and the bitfield output are all unchanged.

Not attempted, not costed. Recorded as the candidate because it is the only option identified so far that
changes the exponent rather than the constant, and because it reuses the existing bucket structure instead of
replacing the renderer. The gather was chosen deliberately on 2026-07-21 over a scatter — but for a reason
specific to **OR-ing bits into a bitfield** (GL cannot bitwise-blend, forcing ping-pong), which does not apply
to filling a per-tile caster list on the CPU. That same document also predicted this exact cliff:
*"the inner caster loop is the one perf cliff (a tile under many lights, each with a long list)"*.

## I5 — Cold and hot are not two lighting methods. They are two accumulators on one path. {#i5}
_2026-07-26 · measured_

**Question:** which of the two implemented lighting methods carries the most large-reach moving lights at the
lowest cost?

**Answer: neither — they are the same method, and the difference is noise.** The only two lighting paths live
in the code today are the **cold** and **hot** classes, and `tick` drives them through *the same function*:

```ts
this.classPass(0, this.coldDirtyTex, this.coldShadowRT!, this.coldLightRT!, this.coldShadowPrevRT);
this.classPass(1, this.hotDirtyTex,  this.hotShadowRT!,  this.hotLightRT!,  this.hotShadowPrevRT);
```

Same shaders, same walk, same RT dimensions (both 512 × 256 shadow / 4096 × 2048 lightmap, read off the GL
objects). They differ only in **which dirty texture gates them** and which accumulator they sum into.

Measured at zoom 0.5, reach 16 tiles, every light orbiting, light count asserted against
`carriedLights.size` (= asked + the 3 content torches):

| moving lights | cold | hot | Δ |
|---|---|---|---|
| 11 | **13.57 ms** (74 fps) | 13.91 ms (72 fps) | +2.5 % |
| 35 | **41.22 ms** (24 fps) | 44.30 ms (23 fps) | +7.5 % |
| 67 | **75.15 ms** (13 fps) | 79.40 ms (13 fps) | +5.7 % |

Cold is consistently a few percent *faster*. Cost is **linear in moving-light count** (~1.1–1.2 ms per
moving reach-16 light at this zoom), so the 60 fps ceiling here is **≈13 moving lights at reach 16**.

**What the split actually buys is isolation, not throughput:** a hot mover invalidates only the hot
accumulator, leaving the cold bake of the static lights untouched. That matters for a scene of *many static +
few moving* lights — but it does not make an individual mover cheaper, and the numbers above say a mover costs
the same either way.

**So the ceiling is not set by picking a class.** By [I4](#i4) it is set by `lit-texels × walk-tiles`:
- **light count** — linear. Static lights are ~free (123 static held 120 fps at 0.72 ms), so the entire bill
  is motion.
- **reach** — superlinear (≈R² coverage × R walk), which is why it dominates and why 16 → 8 was worth 6.7×.
- **the 2.79× oversample** — a flat multiplier on everything, and the one lever that costs no light quality
  ([P4](todo.md)).

_Caveat on scope:_ this compares the two classes that exist **now**. Earlier lighting rebuilds (the coarse
aggregate-direction lightmap that [lightmap-fine-per-light](../2026-07-24-lightmap-resolution/README.md)
replaced) were deleted rather than kept behind a flag, so they cannot be re-measured — only re-implemented.

_Harness note:_ a first attempt at this comparison reported both classes at the 122 fps floor with
`moversReallyMoving: false` — the lights had never attached, because the trial reused a prim array captured
before a zoom change. It measured an idle renderer twice. The assertion caught it; without
`registered` and the movement check it would have read as "the two methods are identical", which is the right
conclusion reached from no evidence at all.

## I4 — The real cost model, and the 2.8× lightmap oversample {#i4}
_2026-07-26 · **the third and (finally) evidence-supported model.** Supersedes the cost claims in
[I2](issues.md#i2), which are wrong._

**User challenge:** _"Zooming in should reduce the work the shader needs to do. 8192 tiles is more difficult to
compute than 512. So we are doing something we are not supposed to be doing, and it scales with px not tiles."_

### The controlled experiment
Every buffer is **constant** across zoom — shadow RT 512 × 256, lightmap RT 4096 × 2048, measured. So holding
the dirty set at 100 % isolates zoom itself:

| zoom | reach (tiles) | world tiles under the map | dirty | ms |
|---|---|---|---|---|
| 1.0 | 8 | 512 | all | 19.66 |
| 0.25 | 8 | **8 192** | all | **8.25** |
| 0.25 | 32 | 8 192 | all | **63.25** |

Row 2 has **16× more world tiles than row 1 and is the cheapest**. Row 3 has the *same* tiles as row 2 and
costs **7.7×** more. **The map's tile count does not enter the cost.**

### The model the evidence supports
> **`cost ∝ (texels a light covers) × (tiles the corridor walks per texel)`**
> — and a light covers `π·R²ₜᵢₗₑₛ × texels_per_tile`, where `texels_per_tile = 16 384 / 4^lod`.

Checks against all three rows: row 1 saturates the 8.4 M-texel map at walk ≈ 2R = 16 tiles. Row 2 covers
`3 × π·64 × 1 024 ≈ 618 k` texels — 13.6× less — at the same walk, so it lands under the 8.33 ms vsync floor.
Row 3 saturates the map again *and* quadruples the walk (R 8 → 32), predicting ~4× row 1: 19.66 × 4 = 78 vs
**63.25 measured**.

**Both earlier models were wrong.** The first said cost tracks the dirty fraction — but row 2 has 8 192 dirty
tiles and is the cheapest run of the whole session, because **a dirty texel with no light in range is nearly
free**. "Dirty" means *recompute*, not *expensive*. The second said reach dominates — true, but as a symptom:
reach enters through both terms of the real model, which is why it looked like the cause.

### So is zoom-in doing something it shouldn't?
**Not in the way the tile count suggests — but yes, there is real waste, and it is exactly per-px.**

Zoom-in is expensive because a world-space light covers **4× more of the fixed-size lightmap per lod step**.
An 8-tile torch is 8 tiles wide at every zoom; at lod 0 those tiles are 128 texels each, at lod 2 they are 32.
That part is inherent: a light that fills your screen costs a screen of lighting, and no addressing scheme
changes it.

**The waste is that "a screen of lighting" is 2.8× larger than the screen:**

| | px | vs canvas |
|---|---|---|
| lightmap RT | 4096 × 2048 = **8.39 M** | **2.79×** |
| canvas | 2560 × 1172 = 3.00 M | 1.0 |

Two deliberate decisions multiply: the lightmap is 1:1 with the **fixed 3584 × 1536 reference** (5.5 M) rather
than the actual canvas — 1.83× on this display — and it spans **32 × 16 slots against 28 × 12 visible** for
pan overscan — 1.52×. Each is defensible alone ([textile-slot](../2026-07-26-textile-slot/README.md) chose the
reference so every player sees the same world; overscan is what lets a pan avoid a re-bake). Together they
mean **we compute 2.79 lighting texels for every pixel we display**, at every zoom.

That is the "something we are not supposed to be doing, and it scales with px". The reference resolution
should govern **what world is visible**, not **how many texels we integrate** — those are welded together
today and need not be. Sizing the lightmap to the canvas would cut lighting cost by up to 2.79× on this
display, with no change to what the player sees. Deferred to [P4](todo.md); not attempted here.

## I1 — A moving light dirties only its NEW reach box, leaving the old one baked {#i1}
_2026-07-26 · open (fixed in [P1](todo.md))_

`markLightDirty(L, from?)` exists precisely so a move can queue the **union of old ∪ new reach**
([`shadowGather.ts:1400`](../../../client/webgl/src/game/viewport/shadowGather.ts)) — the `from` parameter is
documented as "pass when the light moved". The one call site that fires on a light change does not pass it:

```ts
if (cl.changed) {
  const w = this.coldData.carriedLights.get(cl.id)!;
  this.markLightDirty({ x: w.x, y: w.y, reach: w.reach, dynamic: p.light.hot });  // ← no `from`
}
```

`markLightMove` then uses `from?.x ?? L.x` for both ends, so the queued rect is just the **new** reach box.
Everything the light used to illuminate and no longer does keeps its baked value.

Why it was invisible: nothing has ever moved. The lights are static content torches, `stepOrbit` wrote a
discarded read model ([I40](../2026-07-25-primitive-graph/issues.md)), and the debug light array that the old
`__orbit` drove was deleted by [primitive-graph](../2026-07-25-primitive-graph/README.md). So the only code
path that could expose the bug has never executed. **A parameter that exists for a case that has never run is
not tested by anything** — worth remembering when the next optional argument goes in.

Expected symptom once movement works: a smear trailing a moving light, worst on a fast mover, and *nearly
invisible* on a slow one because consecutive reach boxes overlap heavily. That near-invisibility is the
dangerous part — it would read as "shadows are a bit laggy" rather than as a correctness bug.

Fix: pass `from` (the light's previous position, which `carriedLights` already holds before the update).

## I2 — The zoom-in cliff (two superseded cost models — kept for the record) {#i2}
_2026-07-26 · **the CONCLUSIONS below are WRONG. See [I4](#i4) for the model the evidence actually supports.**_

The observations here are sound and the fix that came out of them is real (reach 16 → 8 took zoom 1 from 23 to
120 fps). The *explanations* are not. Two successive models both failed, and the way each failed is the useful
part:

1. **"cost ∝ dirty fraction"** — fitted the zoom sweep to 1.3 %, because that sweep held reach constant, making
   the fraction the only variable. Refuted by reach 16 vs 12 dirtying the identical 512 tiles at 1.65× apart.
2. **"reach dominates"** — true but a symptom, not a cause. Refuted as an explanation by zoom 0.25/reach 8
   being the *cheapest* run of the session with 8 192 dirty tiles: **a dirty texel with no light in range is
   nearly free**, so the dirty count was never the work.

Both were fitted to a sweep that varied one input and then stated as laws. Left here unedited because the
sequence — plausible model, confirming sweep, refutation by an input the sweep never varied — is the same
shape as [I3](#i3)'s invented mechanism, twice more.

### Measured (P0, 2026-07-26 · 3 orbiting torches, reach 16, `focus=104,55`)

| zoom | lod | map (tiles) | dirty/frame | **dirty fraction** | ms static | ms moving | fps moving |
|---|---|---|---|---|---|---|---|
| 1.0 | 0 | 32 × 16 = 512 | 512 | **100.0 %** | 8.33 | **43.06** | 23 |
| 0.5 | 1 | 64 × 32 = 2 048 | 1 363 | 66.6 % | 8.33 | 28.34 | 35 |
| 0.25 | 2 | 128 × 64 = 8 192 | 1 811 | 22.1 % | 8.33 | 8.33 | **120** |

**The hypothesis is confirmed and the mechanism is exactly as predicted at zoom 1: the dirty fraction
saturates at 100 %.** Three reach-16 lights each claim a 32 × 32 = 1 024-tile box against a 512-tile map, so
the union is the entire map and there is no zoom further in where it improves.

**Correction to the cost model I wrote before measuring.** I said per-tile cost was constant and the tile
count was what moved. Both halves were wrong, and they were wrong in *opposite* directions, which is why the
conclusion survived: zooming out bakes **3.5× MORE tiles** (512 → 1 811) in **5.2× LESS time**. A tile at lod
0 fills a whole slot; at lod 2 it fills 1/16 of one, so per-tile texels fall 16× while the tile count rises
3.5×. The right statement collapses both:

> `work ∝ dirty_tiles × texels_per_tile = (fraction × slots × 4^lod) × (slot_texels / 4^lod)`
> **`= fraction × (slots × slot_texels)`** — a **constant texel budget**, of which the dirty fraction is
> re-baked each frame.

Checked against the measurements, taking zoom 1 as the reference: 43.06 × 0.666 = **28.7** predicted vs
**28.34** measured (**1.3 %**); 43.06 × 0.221 = 9.5 predicted vs 8.33 measured, which is the 120 fps vsync
floor, so the true value is at or under it. The static row is 8.33 ms at every zoom — the same floor —
confirming the bake is genuinely idle when nothing moves.

### The model is incomplete: REACH is the dominant term, not the fraction

A second sweep, varying **reach only** at zoom 1, breaks the fraction-only model:

| reach (tiles) | dirty | fraction | ms | fps |
|---|---|---|---|---|
| 16 | 512 | 100 % | **55.63** | 18 |
| 12 | 512 | **100 %** | **33.71** | 30 |
| 8 | 423 | 82.6 % | **8.32** | **120** |
| 4 | 263 | 51.4 % | 8.33 | 120 |
| 2 | 141 | 27.5 % | 8.33 | 120 |

**Reach 16 and reach 12 dirty the identical 512 tiles and differ by 1.65×.** So cost is not a function of the
dirty fraction alone, and the clean `work ∝ fraction × budget` statement above is wrong as a general law. It
was only valid *within the zoom sweep*, where reach was held at 16 and the fraction was therefore the sole
variable — which is exactly why it fitted to 1.3% and exactly why that fit did not generalise. **A model
validated against a sweep that varied one input is a model about that input, not a law.**

Reach enters the cost **three times over**, which is why it dominates:
1. the corridor walk runs from a texel toward its light, so **walk length ∝ reach**;
2. the texels a light claims go as **reach²** (until they saturate the map, as at reach ≥ 12 here);
3. more reach means **more lights overlap each texel** — at reach 16 all three torches reach every texel of
   this map, so `accumulateLights` runs 3 walks per texel instead of 1.

The measured curve is steeper than any of these alone, consistent with all three compounding.

**The practical headline: at zoom 1, reach 16 → 8 takes three moving lights from 55.6 ms (18 fps) to the
120 fps vsync floor.** That is ≥6.7× and it clears P2's ≥60 fps target on its own.

**So the user's "something per-px is incorrect" is right about the effect and wrong about the location.** No
shader does more work per texel as you zoom in. The texel budget is constant by construction (that is what
[textile-slot](../2026-07-26-textile-slot/README.md) bought); zooming in shrinks the world under that fixed
budget until one torch's reach covers all of it.

**Also settled: the dirty counter is per-frame, not latched.** It reads **0** in every static row. The
"constant 696" recorded in [`completed.md`](completed.md) as a suspected latch was a stale reading — that
caveat is closed.

**Corollary, now with numbers.** At zoom 0.25 three moving lights are *free* (120 fps, identical to static).
At zoom 1 they cost 43 ms. Nothing about the lights changed — only how much of the visible world each one
claims. A reach of 16 tiles against a 28 × 12-tile screen is not a torch, it is ambient light
([F3](forks.md#f3)).

### The original prediction, kept for the record
_2026-07-26 · written before measuring_

**Observation (user):** zooming all the way in tanks performance, which is backwards — less world on screen
should mean less work.

**Hypothesis.** Since [textile-slot](../2026-07-26-textile-slot/README.md) every map is a **fixed 32 × 16 slot
grid** whose texel count never changes. A slot holds `2^lod` tiles per edge, so the *world* under the map is
512 tiles at lod 0, 2 048 at lod 1, 8 192 at lod 2. A reach-16 light's dirty box is **1 024 tiles at every
lod** — it is world-space and zoom has no opinion about it. So the fraction of the map one moving light
invalidates is:

| zoom | lod | world covered | dirty fraction |
|---|---|---|---|
| 1.0 | 0 | 512 tiles | **100 %** (the box is 2× the map — it saturates) |
| 0.5 | 1 | 2 048 tiles | 50 % |
| 0.25 | 2 | 8 192 tiles | 12.5 % |

Per-texel cost is *constant* (constant texel count, and the corridor walk is measured in world tiles). What
grows is how many of those texels are re-baked per frame: 3 moving lights at zoom 1 = **3 whole-map bakes
every frame**, and there is no further-in zoom where it recovers, because it is already saturated.

**So the user's "something per-px is incorrect" is right about the effect and, I believe, wrong about the
location:** no per-px shader is doing more work per px — there are simply more px paying the same price.

**Why this is written as a hypothesis and not a finding.** It is *plausible arithmetic I have not measured*,
and plausible-but-unmeasured is precisely what
[I37](../2026-07-25-primitive-graph/issues.md#i37) was — a light height of 0.6 tiles looked entirely
reasonable and silently zeroed every shadow in the world. P0 measures the actual dirty fraction at three
zooms. If it does not track 100/50/12.5%, the model is wrong and P2 gets re-planned rather than built on top
of a nice-sounding table.

**Corollary if it holds:** a reach of 16 tiles against a 28 × 12-tile visible area is not a torch, it is
ambient light. The reach was chosen for how it looked while zoomed out and nothing tied it back to the screen
— see [F3](forks.md#f3).

## I3 — A carrier prim's POSITION record is written once, at allocation, and never again {#i3}
_2026-07-26 · **ROOT CAUSE CONFIRMED** (fixed in [P1](todo.md)) — supersedes the diagnosis carried in from
[primitive-graph I40](../2026-07-25-primitive-graph/issues.md), which was wrong; see the correction below_

**The bug.** [`coldShadowData.ts:646`](../../../client/webgl/src/game/viewport/coldShadowData.ts) writes a
carried light's carrier prim position **only on the frame the carrier is allocated**:

```ts
let prim = this.primOfBillboard.get(billboardId);
if (prim === undefined) {                                   // ← ONLY here
  prim = this.allocPrim();
  this.primOfBillboard.set(billboardId, prim);
  this.writeRecord(PRIM_BASE + prim, encodePosition(...), ...);   // ← the only position write
}
return this.writeCarriedLight(billboardId, prim, L);
```

Every later frame finds `prim` in the map and skips the branch. `writeCarriedLight` then rewrites the record
as `writeRecord(PRIM_BASE + prim, m[pb], G, …)` — passing `m[pb]`, the **existing** R word — so it explicitly
preserves the stale position. Downstream, `resolveCarried` resolves the light against that frozen carrier, so
`decodePosition(r.pos)` returns the same `wx, wy` forever, `moved` is false, `changed` is false, and
`markLightDirty` never fires.

**So the prim graph's position record has exactly one writer: allocation.** That is what "there is no method
to move a prim" means concretely — not a missing notification, a missing *write*. A moved prim's sprite
follows (the albedo cache re-reads `prim.x/y` directly), which is why movement has always *looked* half-real:
the torch slides, its light stays nailed to where it was first seen.

**Correction — the diagnosis I recorded earlier was wrong.** I40 concluded that `stepOrbit` "writes a read
model that is discarded", on the theory that `lastStanding` is a per-frame snapshot. The list is, but
[`SquareCache.standingPrims()`](../../../client/webgl/src/game/viewport/SquareCache.ts) does
`for (const { prim } of this.prims.values()) out.push(prim)` — it pushes **references to the stored prims**.
So `stepOrbit`'s writes land on the authoritative objects after all, and the whole "discarded read model"
story was false. The observation it was invented to explain (map bit-identical, orbit on and off) was real;
the mechanism was not.

Two things to keep from that:
- **A confirmed observation plus an unverified mechanism is still an unverified mechanism**, and it is more
  dangerous than an open question because it reads as settled. I wrote this diagnosis into `README.md`,
  `issues.md` and the work index before checking one 4-line method.
- The generalisation from I37/I39/I40 survives intact and is why the real cause turned up: **a proxy near the
  START of a pipeline is not evidence about its END.** The output — a bit-identical shadow map — was right
  all along; only my explanation of it was wrong. Hence the acceptance rule at the top of [`todo.md`](todo.md).
