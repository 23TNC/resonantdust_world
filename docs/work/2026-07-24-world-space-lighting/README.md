# World-space lighting — light the TILTED world, not the flattened screen — 2026-07-24

_Ground tilt is now a single live dial (`__tilt(deg)`, data-map). **Default is 45°** (chosen from the user's
Fusion360 3D model, 2026-07-24 — 65° was an empirical shadow-look pick from before the lighting was honest
3D). The `65°`/`cos65` in the derivation below are illustrative; the model is `cos θ` for whatever `θ` the
art wants._

_Component: [`client/webgl`](../../components/client/) · `game/viewport/` — the lighting pass (`LIGHT_FRAG`
in [`shadowGather.ts`](../../../client/webgl/src/game/viewport/shadowGather.ts)) + the display blit relief.
Phases in [`todo.md`](todo.md); decisions in [`forks.md`](forks.md); findings in [`issues.md`](issues.md).
Builds on [`2026-07-23-world-geometry`](../2026-07-23-world-geometry/README.md) (the ratified `z = sin65·Δ`
model) and meets [`2026-07-24-normal-tilt`](../2026-07-24-normal-tilt/README.md) at the `N·L`._

## The realization (user, 2026-07-24)
All our lighting math runs in **flattened screen space**, but the world is a plane tilted **65°** to the
view. Screen space is anisotropic about the world: the **east–west (x)** axis is 1:1, but the **north–south
(y / tilt)** axis is **foreshortened** (a screen-y step covers more true ground than an equal screen-x step).
Everything the lighting computes as a flat 2D quantity therefore inherits that lie. Two symptoms, one cause:

1. **Falloff is a circle; it should be an ellipse (oval).** We compute `dist = length(Lxy − P)` as a
   screen-space radius, so the light's reach is a screen circle. In the true tilted world that circle is an
   **ellipse flattened along N–S** (~by the tilt factor), and it also ignores the light's **height** (a point
   directly under the light is at distance `Z_light`, not 0). The reach is really a **3D distance**; the oval
   is its shadow on the screen.
2. **Normal `N·L` dots a world normal against a screen-space light direction.** The
   [`normal-tilt`](../2026-07-24-normal-tilt/README.md) work pitches the baked normals down into the world
   frame (perpendicular to the tilted ground) — correct — but the relief still dots them against a
   **screen-space** `toL/dist`. One operand is in world space, the other in screen space, so the `N·L` is a
   mismatch. Standing billboards especially: a billboard turned *away* from a light should read dark, and
   today that isn't represented.

## The unified fix
Build the **light→point vector once, in true world 3D**, per texel — N–S un-foreshortened, plus the light's
**height** `Z`, plus (for billboards) the **point's own fictional elevation** — then use:
- its **length** for the falloff (⟹ the oval + the height term, both correct), and
- its **direction** for `N·L` against the world-frame normal (⟹ correct relief, and the macro front/back-lit
  darkening of standing prims falls out for free — the `prim.y > light.y` special case from the design chat
  is just `N·L < 0`).

One correct quantity, three fixes (oval falloff, world-space relief, standing-prim shading). This is the
physically-honest version of the emission + relief we already bake; it does not touch the shadow gather or
its corridor↔brute identity.

## Why this is worth doing
- It replaces three separate approximations (radial falloff, screen-space relief dir, a bespoke front/back
  prim-darkening) with **one** world-space computation that is correct by construction.
- It composes with the ratified geometry model and the normal-tilt work rather than fighting them.
- It is testable + reversible: gate world-space lighting behind a toggle and **A/B against the current
  screen-space path** at every step.

## The one real risk — get the trig right, don't eyeball it
The exact transform (the N–S foreshorten factor — `cos65` vs `sin65` vs something tied to the fictional
height; how the light `Z` enters; how a **ground** point vs a **billboard** point un-projects) is precisely
the class of sign/factor call that cost a day on world-geometry (the retracted `2·tan65` misread —
[`world-geometry issues I-1/I-5`](../2026-07-23-world-geometry/issues.md#i1)). So **P0 is to derive and
WRITE DOWN the model** from the ratified `z = sin65·Δ` geometry and **confirm it against the user's
simulation** before any shader code — same discipline that made world-geometry converge.

## Scope / non-goals
- In scope: the **falloff distance** and the **light direction** used by emission + relief, computed in world
  3D. First for the debug light rig.
- Out of scope (for now): re-deriving the **shadow projection** (it already runs its own world-tilt model in
  `shadowCover`); changing the baked normals (that's [`normal-tilt`](../2026-07-24-normal-tilt/README.md));
  warm/mover receivers.
- Open look-call it enables: whether to keep the **additive** relief (`1 + gain·dot`, which can exceed 1 and
  makes the bright-spots-in-shadow the user flagged) or switch to a **clamped Lambert** now that `N·L` is
  world-correct ([forks F4](forks.md#f4)).
</content>
