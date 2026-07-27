# Issues — plane intersection

_Problems hit, candidate solutions, which we chose and why._

## I1 — `casterCover` answers the same question twice

The stream's founding finding (user, 2026-07-27: _"each and every point is casting the full quad and then
checking if they're in that quad. That… is ridiculous."_).

`casterCover` decides "is P shadowed by this caster" with **two independent implementations, in series**.

**First, `shadowCover` works FORWARD** — projects the caster's extremes to the ground and tests whether
`P` falls between them. No quad object exists; `bl`/`br`/`tl`/`tr` are four `vec2` locals alive for four
lines (user, 2026-07-27: _"We don't have a projection quad. We ran math to figure out where we are."_).
Per caster, per texel:

| work | varies with P? |
|---|---|
| `fetchLin` billboard record, `fetchLin` definition record | no |
| `worldTiltRad` — a third `fetchLin`, returning a PASS-CONSTANT | no |
| decode W, H, span, offsets, lod, anchors → `Ac` | no |
| `Yt = A.y − 0.5·H·ct`, `Zt = H·st`, `Yb` | no |
| `k = L.z/(L.z − Zt)` | no |
| `projectTop` ×2 — each a `length()`, a divide, a compare | no |
| corners `bl, br, tl, tr` | no |
| `d0..d3 = cross2(edge, P − corner)` | **yes** |
| sign test → 1.0 / 0.0 | **yes** |

Everything above the line is bit-identical for every texel that caster's shadow covers. A caster covering
a 20 × 60 patch reconstructs the same quad ~1200 times per light.

**Then, if that passed, the tap loop re-derives the same fact:**

    float denom = H * (st * (P.y - Lp.y) - 0.5 * Lp.z * ct);
    float t = Lp.z * (P.y - Ac.y) / denom;   // where up the card the ray crosses
    if (t < 0.0 || t > 1.0) continue;        // missed vertically -> lit
    float k = Lp.z / (Lp.z - t * H * st);
    float s = ((P.x - Lp.x) / k + Lp.x - Ac.x) / W + 0.5;
    if (s < 0.0 || s > 1.0) continue;        // missed horizontally -> lit

That is the light→P ray intersected with the tilted card, range-checked — the plane intersection the
proposal describes, already written, already correct for the tilt. For the CENTRE sub-light the two are
the same predicate.

**Why it is worth more than it looks.** The quad build is paid on MISSES, and misses dominate: the
corridor visits up to 8 casters per tile over ~20 tiles and only a handful shadow any given texel. Every
other one pays the full construction to return 0. This is consistent with
[moving-lights I11](../2026-07-26-moving-lights/issues.md) measuring `casterOne` at 70 % of frame and the
walk as 77 % ALU rather than fetch-bound — the arithmetic above the line is what that is.

**Not yet proven, and P0 exists to prove it:** that the two predicates agree *exactly*. Three known
reasons they might not — `projectTop` clamps to `reachU`; `SHADOW_BASE_PUSH` shifts the base south in the
quad only; the `t`/`s` bounds are half-open where the `cross2` test uses `>= 0 || <= 0` on both signs.
Each must be given a named cause before anything is deleted. Deleting first and diffing after would make
a behaviour change indistinguishable from a bug.

## I2 — the flattened-2D reading was mine, not the proposal's

Logged so it is not re-introduced. On first pass I objected that the proposal's snippet treats the
billboard as a horizontal segment at `billboard.y`, which would ignore the tilt and land shadows wrong.
The user corrected it: the sketch is the **flattened on-screen shadow**, not a plan-view model. The base
is anchored so the shadow always starts at the base; height shifts the FAR endpoints outward, left of
centre to the left and right of centre to the right. The quads and triangles remain — they stretch.

The consequence for this stream: **keep the existing tilted-card inversion verbatim.** The change is
which question we ask, not the geometry we ask it about. Any patch here that introduces a fresh 2D
intersection instead of reusing the `denom`/`t`/`k`/`s` block is wrong.

## I3 — the inversion yields the UV, not just the hit test

User, 2026-07-27, sharpening [I1](#i1):

> _"Because you're calculating 'where up the card' the point hit, I believe this becomes our v in our uv
> lookup. I believe we have the u as well from the x intersection. So I believe this alleviates the second
> pass we were doing."_

Confirmed in the code — the very next line after the range checks builds the lookup from them:

    vec2 uv = vec2(fx, fy) + vec2(ox, oy) * ppu - vec2(nx, ny)
            + vec2(s * W, (1.0 - t) * H) * ppu;

`s` is the u; `(1 - t)` is the v, flipped only because atlas rows run image-top-down while card `t = 0` is
the sprite's bottom row.

**Why this strengthens the case for deleting the quad.** I1 framed it as the same predicate computed
twice. It is worse than that: the surviving computation is one we must perform ANYWAY to sample the
silhouette. One solve yields two results — the containment answer AND the texture coordinate. The quad
test yields only the boolean and discards the geometry, so it shares NO work with what follows. It is not
a cheaper pre-filter; it is pure overhead.

Counted properly there are **four** containment tests for one question: the quad's four `cross2` signs,
then `t` in range, then `s` in range, then the `uv` frame-bounds check.

## I4 — deleting the quad without HOISTING makes misses worse

A trap in the first draft of P1, caught before implementation.

The `(s,t)` inversion currently lives INSIDE the tap loop, and each tap `continue`s independently on a
miss. So simply removing `shadowCover` would make a total miss cost **N inversions** — one per tap — where
today it costs one quad test and then N inversions. The deletion only pays if the centre-ray inversion is
hoisted to a single gate ahead of the loop:

| | today | deleted only | deleted + hoisted |
|---|---|---|---|
| miss | quad build + N inversions | N inversions | **1 inversion (~6 ALU)** |
| hit | quad build + N inversions | N inversions | 1 gate + N inversions |

Misses dominate, so the hoist is the entire win, not an optimisation on top of it. P1 is worded to hoist
first and delete second for this reason.

Note this concern evaporates at P3, where the whole loop collapses to two rays — but P1 must stand on its
own as a bit-identical refactor, so it cannot lean on a later phase to rescue its cost.

## I5 — a resize schedules an unbounded single-frame rebake, and it kills the GPU

Found while A/B-ing the card lean, 2026-07-27. Not caused by this stream, but it will bite anyone running
P0–P2's verification, so it is recorded here.

**Symptom.** The page renders white with a broken-image glyph; `gl.isContextLost()` is `true` and
`gl.getError()` is `37442` (`CONTEXT_LOST_WEBGL`). Chrome has killed the renderer process.

**Mechanism.** The client loads with the **Chat** panel selected, so the game canvas is hidden and sized
`1×1` (`camera` reports `0×0`). Selecting **Game View** resizes it to the real viewport, and the resize
path issues a full rebake of every tile *in one frame*. With three reach-20 torches — and longer shadows
once the lean went to 1.0 — that single draw runs long enough to trip the GPU watchdog.

**Why it kept getting misdiagnosed.** A hidden canvas produces the SAME blank screen as a lost context,
and neither logs an error. Twice I attributed the blank to the wrong cause — once to reach 20 (and edited
content on that guess, since reverted), once to a shader change. The distinguishing check is one line, and
it should be the FIRST thing run on any blank frame, before forming a hypothesis:

    gl.isContextLost() + ' ' + camera.width + 'x' + camera.height + ' ' + gl.canvas.width

`0×0` / `1×1` means the panel is hidden — not a crash. `isContextLost() === true` means the GPU died.

**Workaround for testing.** Load with Game View already the active tab so the canvas is sized before the
first bake; the same scene that crashes on resize is stable when it never resizes.

**The real bug, for a separate stream.** A resize must not be able to schedule an unbounded rebake. The
work is already tile-granular and the prioritised refinement queue already exists
([textile-slot P5](../2026-07-26-textile-slot/todo.md)) — a resize should enqueue tiles and drain them
across frames rather than issue one draw whose length scales with window area × lights × reach. Until it
does, reach is bounded by watchdog latency and not by the fps budget, which is a much lower ceiling and an
invisible one.

## I6 — I5's diagnosis was WRONG, and the budget "fix" is unvalidated

Retracting most of [I5](#i5) the same day it was written.

**What I5 claimed:** a resize marks every slot dirty in one frame, the gather is a single draw, so the
draw grows without bound and trips the GPU watchdog. **Fix:** budget the slots per frame.

**Why that is wrong:**

- `SquareCache.bakeDirty(budget: number)` **already takes a budget** — the G-buffer path was never
  unbounded. I asserted it was without reading the signature.
- After implementing the shadow-side budget (`bakeBudget`, 192 slots/frame), the context was **still
  lost** on the very next panel switch. The fix did not fix it.
- `camera` was **already** `1862×853` before the click that killed it, so that instance was not a resize
  at all. The panel switch alone did it.
- I told the user a Chrome restart was needed because the GPU process had degraded across context
  losses. Immediately after a fresh Chrome, the first panel click killed the context again. That
  explanation is also unsupported.

**What is actually known** — and this is a decent bug report even without a cause:

- Trigger: clicking the **Game View** tab reliably loses the WebGL context (`isContextLost() === true`,
  `getError()` `37442`). Reproduced ~6 times across two Chrome sessions.
- Loading with Game View already active is fine; the page is healthy and renders.
- Both bake paths are budgeted, so "one enormous draw" is not the mechanism.
- Not yet ruled out: RT churn on panel switch (destroy/recreate), VRAM exhaustion, or a driver fault
  unrelated to draw length.

**Risk I introduced and have NOT verified.** `bakeBudget` carries work across frames via `pendingWork`.
If the render loop is change-gated rather than continuous, pending slots may never drain and shadows
would stay permanently incomplete — the change-gated-render-loop gotcha that has bitten this codebase
before. The budget is bounded-work-per-frame, which is defensible on its own merits, but it was built on
a wrong diagnosis and its drain path is unproven. `__bakebudget(0)` restores the old behaviour, and
reverting it entirely is reasonable.

**Lesson, and it is the same one as I1/I40:** I had a confirmed observation (context dies) and invented a
mechanism (unbounded draw) that fit it, then built a fix on the mechanism without testing the mechanism.
One `grep` for `bakeDirty`'s signature would have killed the theory before any code was written.

## I7 — blockers reclassified (user, 2026-07-27): nothing is actually blocking

I filed three rows in `blockers.md`. The user reclassified all three, and the file is deleted — a blocker
is something that needs human input to proceed, and none of these do.

**B1 — the SDF corpus re-bake.** Not a blocker, a SEQUENCING decision: _"We can handle the wedge coloring
last. We can work through how we texture the shadows then."_ So P4 moves to the END of the stream, after
P5, and the shadow-texturing question gets worked through as its own thing rather than gating earlier
phases. [F5](forks.md#f5) already holds the option analysis.

**B2 — torch reach 8 is a safety value, not an art call.** User: _"This doesn't sound like a blocker this
sounds like a note or issue."_ Correct. Recorded here: reach 8 is the measured-good value (120 fps with
3 torches; 16 → 18 fps) and it is authored for what the renderer can afford, not for how the world reads.
If P3 raises the affordable ceiling, the art question reopens — but nothing is waiting on it.

**B3 — the Game View click loses the WebGL context.** User: _"We will certainly need to debug if this
keeps happening."_ So: a WATCH item, not a blocker. Workaround is reliable (load with Game View already
active).

One correction, because the two symptoms are getting merged. The user wrote _"You believe it is coming
from streaming data"_ — that was my read of the **slow load** (things trickling in behind tiles, which
the user then attributed to a docker container hiccup, and which fits better). It is NOT my read of the
context loss. Those are separate: the slow load is the app running fine while data arrives late; the
context loss is the GPU dying on a panel switch. I have no working theory for the second, and both bake
paths being budgeted rules out the obvious one. If it recurs, the bisect is to instrument render-target
create/destroy across the panel switch.
