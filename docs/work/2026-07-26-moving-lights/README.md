# Moving prims, and lighting that survives them — 2026-07-26

_Components: [`client/webgl`](../../components/client/) (`game/viewport/shadowGather.ts`,
`coldShadowData.ts`, `SquareCache.ts`, `Viewport.ts`). Phases in [`todo.md`](todo.md); decisions in
[`forks.md`](forks.md); findings in [`issues.md`](issues.md)._

## The decision (user, 2026-07-26)
**Two things, in this order.** First, **implement a real method to MOVE a prim**, tested by moving a light
prim. Second, **work out what lighting we can actually implement**, because the current one is unusable:

> _"Currently 3 lights set to update, which all fail to actually move… cast so many shadows that we're
> dragging our fps down to 30. We have now tried multiple methods. This method of lighting was supposed to
> solve a lot of things by sharing a lot of the work other lights were doing. However by forcing each and
> every shadow to populate all at once for a single light, we seem to have made something unusable."_

And a second observation that turns out to be the sharpest diagnostic we have:

> _"We cannot handle zooming all the way IN. Which doesn't make sense as we should be calculating LESS as we
> zoom in. So there is something in a per-px shader that's incorrect."_

## Why there is no move — root cause, confirmed
**A carrier prim's position record has exactly one writer: allocation.** `carriedLightFor` writes
`PRIM_BASE + prim`'s position *only* on the frame the carrier is created; every later frame finds the carrier
already mapped, skips the write, and `writeCarriedLight` then re-emits the record passing `m[pb]` — the
**existing** position word — so the stale value is explicitly preserved. `resolveCarried` reads that frozen
carrier, so the light's decoded world position never changes, `moved` stays false, and `markLightDirty` never
fires. Full trace in [I3](issues.md#i3).

That is what "there is no method to move a prim" means concretely: not a missing *notification*, a missing
*write*. A moved prim's sprite follows, because the albedo cache reads `prim.x/y` directly — which is exactly
why movement has always looked half-real. The torch slides; its light stays nailed to where it was first seen,
and so does every shadow it casts.

**This corrects the diagnosis I recorded in [primitive-graph I40](../2026-07-25-primitive-graph/issues.md)**,
which blamed `stepOrbit` for writing a discarded per-frame read model. `standingPrims()` returns *references*
to the stored prims, so those writes land fine. The observation was real — the shadow map came back
bit-identical (hash 3750264193, 49 747 non-zero) with the orbit on and off — but the mechanism I attached to it
was invented, and I wrote it into three documents before checking the four-line method that disproves it. The
lesson from [I37](../2026-07-25-primitive-graph/issues.md#i37)/[I39](../2026-07-25-primitive-graph/issues.md#i39)
still holds and is what found the real cause: **a proxy near the START of a pipeline is not evidence about its
END.** Every acceptance in this stream reads the output.

## The lighting cost model — what "sharing" actually bought
The design is right about static lights and the measurements say so. Lights bake into a **shared world-space
accumulator** (additive, quantised to integers so a light can later be subtracted bit-exactly). A static
light bakes **once** and is then free forever, which is why 123 static lights hold 120 fps.

The unit of *invalidation*, though, is a light's **entire reach disc, at full quality, in one frame**. That's
the user's sentence restated: sharing amortises a light across *frames*, but a move throws the whole
amortisation away at once. Measured this session, zoom 0.25, reach 16:

| | ms/frame | fps | dirty tiles |
|---|---|---|---|
| 123 lights, static | **0.72** | 120 | 696 |
| 123 lights, 120 moving | **72.44** | 15.2 | 5 384 |

100× for the same lights, just because they move.

## The zoom-in cliff has a number, and it is not a per-px bug
This is the hypothesis [I2](issues.md#i2) exists to confirm or kill, and it is falsifiable arithmetic:

Since [textile-slot](../2026-07-26-textile-slot/README.md), every map is a **fixed 32 × 16 slot grid** that
never changes size. A slot holds `2^lod` tiles per edge. So the *world* the map covers is:

| zoom | lod | world covered | a reach-16 light's box (32×32 tiles) | **fraction of the map it dirties** |
|---|---|---|---|---|
| 1.0 | 0 | 32 × 16 = **512 tiles** | 1 024 tiles | **100 %** (the box is 2× the whole map) |
| 0.5 | 1 | 64 × 32 = 2 048 tiles | 1 024 tiles | 50 % |
| 0.25 | 2 | 128 × 64 = 8 192 tiles | 1 024 tiles | 12.5 % |

The map's **texel count is constant** — that is what textile-slot bought. So a full-map bake costs the same
milliseconds at every zoom. What changes is **how many full-map bakes a moving light forces per frame**:
0.125 at zoom 0.25, and **1.0 at zoom 1**. Three moving lights at zoom 1 = **three whole-map bakes every
frame**, and there is no zoom level further in where it gets better — it saturates at 100 % and stays.

So zooming in doesn't compute less; it shrinks the world until one torch's reach *is* the world. The user's
instinct that something is wrong per-px is right in effect and, I think, wrong in location — the per-px work
is constant, and the **count of px paying it** is what exploded. P0 measures this before anything is built on
it, because "plausible arithmetic" is exactly what [I37](../2026-07-25-primitive-graph/issues.md#i37) was.

**A reach of 16 tiles is not a torch at zoom 1 — it is ambient light.** The visible area is 28 × 12 tiles.
That reach was chosen for how it looked while zoomed out, and nothing connected it back to the screen.

## What we can actually do — the option set
Full analysis in [`forks.md` F2](forks.md#f2); this is the shape.

1. **Hot lights bake at a coarser lod than cold ones.** The strongest lever, and it costs almost nothing to
   try, because the lod machinery already exists ([textile-slot P4](../2026-07-26-textile-slot/todo.md) is
   literally per-piece lod state). A moving light's shadow is *in motion* — nobody resolves it at 16
   texels/tile. Two lod steps = **16× fewer texels for identical world coverage**. Static lights keep full
   sharpness; only the thing that is already blurred by motion gets blurred by resolution.
2. **Bound reach against the visible world.** Independent of everything else, and it is the honest fix for
   the zoom cliff. Either clamp reach in tiles, or make it lod-aware so a light lights a *place*, not a
   screen.
3. **Amortise the disc over N frames**, nearest-first. Bounded worst case per frame; the light's far field
   lags a few frames behind its near field. Real but modest lag on a slow mover, smearing on a fast one.
4. **Cheap-out the empty corridor.** Most texels in a 16-tile disc have *no caster* between them and the
   light. If those already exit in ~2 fetches, this is nothing; if they don't, the architecture was never the
   problem. **Cheapest thing to measure, so it goes first** — P0 refuses to redesign around a cost we have
   not attributed.

Options 1–3 compose. 4 might make 1–3 unnecessary, which is the whole reason it is measured first.

## What this stream does NOT do
It does not re-litigate the shared-accumulator design. The static numbers vindicate it (123 lights, 0.72 ms),
and three lighting rewrites are enough. The claim under test is narrower and, I think, true: **the sharing
model is correct and the invalidation granularity is wrong.**
