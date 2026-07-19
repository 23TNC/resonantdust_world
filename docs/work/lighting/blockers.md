# Blockers — lighting

_Dependencies gating a `todo` item. Each: what's blocked, why, the plan to clear. Move the item back to
`todo` once cleared (dated resolution line here)._

---

## B1 · Outline serve + consume — RESOLVED 2026-07-18

Edge serves `GET /textures/meta/{stem}` (`serve_meta`) + client `OutlineCache` decodes the earcut
outline; P3 scatter + P4 casters unblocked. Shipped — see [`completed.md`](completed.md).

## B2 · No static-light content source (soft — blocks the cold tier's payoff, not the code) — 2026-07-18

**Partially blocked.** The cold tier exists to amortize **many static lights**, but content authors none
yet (the DSL has no point-light primitive).

**Why.** "Dense many-lights" needs authored cold lights (torches, glows) classified to the cold tier at
worldgen/load. Without them, the `cold_lightmap` bake is real but has nothing to bake.

**Plan to clear.** Not a hard blocker for the *plumbing* — P1's bake + textures can be built and verified
against a **hardcoded / debug** cold light. The DSL point-light primitive + worldgen classification is
**P5**; do it once the pipeline is proven so we're authoring into a working system.

## B3 · Inline cold-occlusion technique unproven (blocks the P4 cold-shadow rework) — 2026-07-19

**Blocked.** The cold strategy ([F7](forks.md#f7)) tests each light's shadow **inline in the bake
fragment shader** — no materialized map. That needs the light's projected caster silhouettes addressable
**per pixel**, and a per-fragment point-in-silhouette test cheap enough to run per light. The encoding +
the simplification budget are not yet worked out.

**Why.** The render `outline` is a ~180-pt contour — far too heavy for a per-fragment point-in-polygon
over several casters × several lights. And a fragment shader can't hold arbitrary geometry in uniforms; it
has to read it from a **data texture** (projected tris/contours per light) or an equivalent structure.

**Plan to clear.** Read how `../resonantdust` actually did the inline sweep (it "passed the prim geo to
the shadow shader" and swept N lights per pixel without writing a map) — the data-texture layout, the
point-in-shape test, and how aggressively the shadow silhouette was decimated vs the render outline. Match
that before building, rather than reinventing. Until then the current materialized `shadow-cold` composite
([D-1](deviations.md)) stands as the interim, capped, build.
