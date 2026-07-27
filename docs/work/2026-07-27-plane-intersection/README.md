# Plane intersection instead of projected quads — 2026-07-27

_Components: [`client/webgl`](../../components/client/) (`game/viewport/shadowGather.ts`). Phases in
[`todo.md`](todo.md); decisions in [`forks.md`](forks.md); findings in [`issues.md`](issues.md)._

## The decision (user, 2026-07-27)

> _"You're saying that each and every point is casting the full quad and then checking if they're in that
> quad. That… is ridiculous. We just need to check if our px's point passes through our billboards plane.
> This determines if there is shadow at our location. […] the most important piece right now, is moving to
> plane intersection instead of projecting our quads."_

And, for the phase after it:

> _"Instead I'd like to cast a shadow from the light +/- radius. But… I don't want to increase our work.
> Instead, we just cast using +radius through the opposite shadow base point, and -radius through the
> opposite shadow base point. This gives us the penumbra in our shadow and solves our cross tile issues.
> […] Every px need only check up to 2 points."_

**A correction the user issued and this stream must not re-introduce.** The 2D sketch in the proposal is
the *flattened, on-screen* shadow, not a plan-view simplification. The base is anchored, so the shadow
always starts at the base; height shifts the FAR endpoints outward — left of centre moves left, right of
centre moves right. The quads and triangles all survive, they **stretch**. So this stream is not replacing
the tilted-card projection with flat 2D maths. It replaces *how we ask the question*, not the geometry.

## The finding that motivates it

`casterCover` answers "is P shadowed by this caster" **twice**, with two different implementations:

1. `shadowCover` builds the caster's whole projected quad — tilt `sin`/`cos`, `Yt`/`Zt`/`Yb`, the `k`
   factor, two `projectTop` calls (a `sqrt` each), four corner positions — then runs four `cross2` sign
   tests to decide containment. Returns 1.0 / 0.0.
2. If that passed, the tap loop **re-derives the same fact** by inverting the light→P ray back to the
   card's `(s,t)` and range-checking `t` then `s`. That inversion IS the plane intersection the user is
   describing, already written, already handling the tilt correctly.

So the quad is a redundant pre-filter in front of the real test. Full trace in [I1](issues.md#i1).

**Why deleting it should win big rather than a little:** the quad build is paid on MISSES, and misses
dominate. The corridor visits up to 8 casters per tile across ~20 tiles; only a handful shadow any given
texel. Every other one currently pays the full quad construction to return 0. Under the ray test a miss
costs a subtract, a divide, a multiply-add and two compares. That is the hot path getting several times
cheaper, and it is exactly where the measured ALU-boundness sits ([moving-lights I11](../2026-07-26-moving-lights/issues.md):
`casterOne` was 70 % of frame, and the walk 77 % ALU rather than fetch-bound).

## Shape of the work

**Replace and delete as we go** (user, 2026-07-27 — [F6](forks.md#f6)). No switches, no dual paths, no
dials: each phase writes the new predicate and removes what it replaced in the same commit. The original
plan kept both behind a uniform and gated on bit-identity; that was tried, cost a day, and is withdrawn.

**P1–P2 replace the quad with the ray solve** and delete `shadowCover`/`projectTop`. **P3 replaces the
tap ladder** with two rays at light ± radius and deletes `emitterOffset` and the tier machinery. **P4–P5**
fill the wedge and pad the caster bucketing.

Because the old path does not survive as a reference, **bit-identity is not the gate** — P0 already
measured the two predicates diverging ~26 %, and that is the intended outcome. The gate is corridor↔brute
identity, three-zoom eyeball, and a faster cold gather. `checkpoint/pre-plane-intersection` is the tag to
diff against.

## What this does and does not solve

| | |
|---|---|
| Removes the duplicate predicate | **yes** — the stream's core |
| Makes misses cheap | **yes** — the dominant path |
| Penumbra outside the hard quad no longer rejected | **yes**, at P3 — the gate becomes the union of the two extreme wedges |
| The tile-boundary hard clip | **half.** The gate is one of its two causes; the other is *bucketing* — a caster is only registered in the tiles its body covers, so a penumbra texel whose corridor misses those tiles never tests it at all. P5. |
| Smooth penumbra across silhouette detail | **no.** Two rays return near-binary silhouette opacity, so "one hits" gives 1/2, not a ramp. Analytic fill works for the card's own edge; interior detail still wants more samples or a distance field. See [F5](forks.md#f5). |

## Guard rails

**Verify at torch reach 8.** Reach 20 put the client past its own measured fps table (16 → 18 fps with
3 torches), which reads as a hang and trips the GPU watchdog — that is what cost 2026-07-27
([I6](issues.md#i6)). To inspect long shadows, move the camera, do not raise reach.

The corridor↔brute identity check is the only thing standing between this class of change and a silent
regression. It is **green as of 2026-07-27 at lean 1.0** (67 437 nonzero texels, 0 differing) and every phase here
re-runs it. Note the baseline moved that day: world-geometry F2 closed and the caster card lean went
`0.5` → `1.0`, so any bit-identity comparison must be against a **lean-1.0** capture, never the older
shipped build. `__corridor(false)` = brute, `__corridor(true)` = corridor, `__gather.debugReadShadow(0)` for
a COLD light — note the argument defaults to `1` (hot), which returns an all-zero buffer for a cold light
and reads exactly like perfect identity.
