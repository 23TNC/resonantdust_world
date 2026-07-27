# Issues — problems hit, candidates, what we chose

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
