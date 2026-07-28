# Intent — lighting feel (staged)

_Feature intent parked per [`CONVENTIONS.md`](../../CONVENTIONS.md) (the components map has no
`client/webgl` entry yet; when it does, this distributes into that component's `intent/`). These
are RATIFIED-BUT-LATER features from the 2026-07-27 lighting-feel conversation — documented so
they aren't re-derived; none are scheduled. The build-now subset lives in
[`work/2026-07-27-lighting-feel/`](../../work/2026-07-27-lighting-feel/README.md)._

## Source halo (user #3 — "document and implement later")

A small additive radial-gradient billboard at each light's screen position — "fire in the air".
Nearly free (the gizmo pass already draws per-light quads; this is its pretty sibling). Belongs in
the display pass, not the lightmaps: it is glare, not illumination — it must not light anything.

## Light shafts (user #8)

Radial accumulation of a light's shadow mask along rays from the source: visible beams through
tree gaps in dark air. Plausible dirty-gated on the COARSE shadow map (131 k texels); real work.
Only worth attempting after the decay lightmap exists (a shaft is, in spirit, a stamped streak —
the decay map may be its natural home).

## Tone curve + ambient grading (user #9 — "worth documenting")

A filmic roll-off in the blit so overlapping light cores compress instead of clipping flat, and a
COLOURED ambient floor (moonless blue-black rather than scalar grey). Constraint: the zero-light
gameplay rule — grading must never lift true black.

## Ground relief (user #4 — no code now)

Ground texels take `ndl = 1` today. When TILE PRIMITIVES carry normal maps through the co-pack,
the existing per-light N·L path lights ground texture directionally with **no new lighting code**
— the receiver map + `billboardNormal` machinery generalise. Do not build a bespoke path earlier.

## SDF-silhouette penumbra (parked pending the user's penumbra call)

The standing proposal for shaped soft shadows, so iteration #6 doesn't start from scratch: five
iterations (taps → rays → wedge → binary search → analytic interval → two-axis) all derived
softness from GEOMETRY and then multiplied by ONE hard silhouette point-sample — which is why
large emitters smear the shape. Instead: bake a signed distance field of each silhouette at
texture ingest; `casterCover` = backward solve → one SDF fetch → `smoothstep(−w, +w, sdf)` with
penumbra width `w ∝ emitter·(k−1)` (from the `invk` already in hand) → optional tip fade `×f(t)`.
Soft in BOTH axes, contact-hard at the base for free, interior gaps dissolve correctly, and it is
CHEAPER per texel than every iteration so far. Note: `t` from the solve IS the normalized
along-shadow position (0 = contact, 1 = tip) — no "shadow length" ever needs computing.

## Particle-emitter prim leaf ([work F5](../../work/2026-07-27-lighting-feel/forks.md#f5) option a)

Once the decay-lightmap look is proven with the CPU emitter, promote emitters to content: a prim
leaf (or a flag lane on the light leaf) authoring flicker/ember behaviour per kind. The splat API
(pos/radius/colour/intensity) is the contract; promotion is additive.
