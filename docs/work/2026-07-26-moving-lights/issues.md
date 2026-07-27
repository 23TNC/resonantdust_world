# Issues — problems hit, candidates, what we chose

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

## I2 — The zoom-in cliff is a dirty-FRACTION effect, not a per-px shader bug {#i2}
_2026-07-26 · **CONFIRMED** — measured, and the corrected model fits to 1.3%_

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
