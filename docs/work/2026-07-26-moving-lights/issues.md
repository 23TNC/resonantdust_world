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

## I2 — The zoom-in cliff is a dirty-FRACTION effect, not a per-px shader bug (HYPOTHESIS) {#i2}
_2026-07-26 · open — P0 confirms or kills it_

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

## I3 — Movement had no entry point, so the orbit "worked" without moving anything {#i3}
_2026-07-26 · open (fixed in [P1](todo.md)) — carried forward from
[primitive-graph I40](../2026-07-25-primitive-graph/issues.md)_

`ShadowGather.lastStanding` is a **per-frame snapshot** rebuilt at the top of `buildCasters` and discarded.
`stepOrbit` mutated `p.x/p.y` on that snapshot. The CPU stamps resolved positions from the **prim graph**, so
the writes went nowhere: the shadow map came back bit-identical (hash 3750264193, 49 747 non-zero) with the
orbit both on and off, and I had already committed a claim that all three torches were "displacing frame to
frame" on the strength of the JS fields changing.

The generalisation, third instance this session: **a proxy near the START of a pipeline is not evidence about
its END.** I37 (authored height looked fine → every shadow zero), I39 (120 lights registered → 0 dirty tiles),
I40 (fields changed → map unchanged). In all three the sound check was one call at the output, and in all
three I did the cheap check instead. Hence the acceptance rule at the top of [`todo.md`](todo.md).
