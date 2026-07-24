# Shadows cast onto primitives — 2026-07-23

_Component: [`client/webgl`](../../components/client/) · `game/viewport/` — a second shadow gather +
a per-texel composite in the lighting bake. Builds on [`2026-07-23-lighting`](../2026-07-23-lighting/README.md)
(the lightmap) + [`2026-07-22-lighting-rebuild`](../2026-07-22-lighting-rebuild/README.md) (the ground
shadow). Phases in [`todo.md`](todo.md); decisions in [`forks.md`](forks.md)._

Make cast shadows land **on standing prims (billboards)** correctly — climbing the billboard at the
right height instead of painting over it flat or being binary-removed as a whole.

## The problem
Our shadow is evaluated on the **ground** (`z = 0`) at every pixel. A pixel on a standing billboard is
NOT on the ground — it's an **elevated** point. Two symptoms of ignoring that:
- **Flat overlay** — a ground shadow painted straight over a sprite (no height); it reads pasted-on.
- **Binary all-or-nothing** — the current default (`__depthmode 1`) un-shadows a *whole* prim when its
  base is in front of the caster. Correct-ish, but the shadow can't *climb* a prim that is genuinely
  behind: it's either fully lit or the ground band cuts across at the wrong height.

## The geometry (converged with the user)
The world is a **65° tilted plane rendered flat** ([[art-style]]). One screen row maps to a ground
point (far up the tilt) AND a billboard point (near) — they collapse only because we flatten the tilt.
A billboard pixel drawn at world `(px, py)` is an elevated point at height `z`, where `z` comes from the
depth map: `z = f(base_row − pixel_row)` (base row from `zdepth-world`, pixel row from the fragment).
Its shadow = whether the light ray **through the elevated point** is blocked by *other* casters —
equivalently, the ground shadow at `G = (px,py)` pushed away from the light `∝ z`.

**Why the cheap re-sample fails** (see [`issues.md`](issues.md)): re-sampling the *combined* ground
shadow at `G` self-shadows — for a lit tree, `G` lands in that tree's OWN ground shadow, so its crown
reads as blocked by its own trunk (the blocker sits between the crown and `G`, not between the light and
the crown). A combined ground-shadow can't tell those apart. The fix needs **per-caster** evaluation
that **excludes the receiver's own (same-tile) caster**.

## The architecture
Both shadows live in the **same** world-space toroidal `shadow-cold` space — a billboard pixel's world
position is exactly where it's drawn, so its shadow belongs at the same texel, just evaluated at `z ≠ 0`
(the key correction — no separate screen/composite space).

- **Pass 2** = the gather again, into a **second** `shadow-cold` (cold/hot, per-light u9), but:
  (1) **early-exit** where the depth map says "no prim" (ground — pass 1 already has it);
  (2) **lift the receiver to `z`** (re-project `G` per light from the depth-map height);
  (3) walk the same caster buckets **excluding casters on the receiver's own tile** (self + same-tile —
      kills the self-shadow). Reuses `casterCover` + the corridor.
- **Composite in the lighting bake** — because both maps are world-space, the lighting pass picks
  **per texel**: billboard shadow where the depth map says a prim is drawn there, ground shadow
  everywhere else. So the blit just samples the lightmap (correct shadow already baked); the
  `__depthmode` binary path stays as a fallback while this comes up.

## What we already have
`zdepth-world` (base row + is-thing, per [[coverage-surface-model]] — now populated), the caster
**buckets** (prim indices) + light records in the unified data texture, `casterCover` + the corridor
walk, the cold/hot **dirty** machinery, and the binary front/behind fallback (`__depthtest`). The
self-shadowing re-sample shortcut was removed ([`issues.md#i1`](issues.md#i1)). Layouts authoritative in
[`docs/VARIABLES.md`](../../VARIABLES.md).
