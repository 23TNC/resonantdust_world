# Issues — plane intersection

_Problems hit, candidate solutions, which we chose and why._

## I1 — `casterCover` answers the same question twice

The stream's founding finding (user, 2026-07-27: _"each and every point is casting the full quad and then
checking if they're in that quad. That… is ridiculous."_).

`casterCover` decides "is P shadowed by this caster" with **two independent implementations, in series**.

**First, `shadowCover` builds the whole projected quad** and point-tests it. Per caster, per texel:

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
