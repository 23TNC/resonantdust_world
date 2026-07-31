# Shadows cast onto primitives — 2026-07-23 · CLOSED (attempt #2, reverted)

> **SUPERSEDED by [`2026-07-24-shadows-onto-prims`](../2026-07-24-shadows-onto-prims/README.md).** This
> attempt was built and reverted (zoom regression — it read the `zdepth` `textile_slot` composite by world
> coordinate). Its [`completed.md`](completed.md) holds the gather math (elevation + cone culls +
> corridor identity) that was correct at a fixed zoom and is reused verbatim by attempt #3. Root cause:
> [`2026-07-24-map-compatibility`](../2026-07-24-map-compatibility/README.md). Read below for history only.

_Component: [`client/webgl`](../../components/client/) · `game/viewport/` — a second gather pass that
writes the **same** `shadow-cold`, presence-partitioned (no composite). Builds on `2026-07-23-lighting`
(the lightmap) + [`2026-07-22-lighting-rebuild`](../2026-07-22-lighting-rebuild/README.md) (the ground
shadow). Phases in [`todo.md`](todo.md); decisions in [`forks.md`](forks.md)._

Make cast shadows land **on standing prims (billboards)** correctly — climbing the billboard at the
right height instead of painting over it flat or being binary-removed as a whole.

> **Depends on [`2026-07-23-world-geometry`](../2026-07-23-world-geometry/README.md).** That stream
> ratifies the one vertical model (`z = sin65·(px.y − base)`) and conforms the caster projection to it.
> Building receivers here while casters still run the old leaning-card fiction would put the two in
> different vertical frames — align first.

## The problem
Our shadow is evaluated on the **ground** (`z = 0`) at every pixel. A pixel on a standing billboard is
NOT on the ground — it's an **elevated** point. Two symptoms of ignoring that:
- **Flat overlay** — a ground shadow painted straight over a sprite (no height); it reads pasted-on.
- **Binary all-or-nothing** — the current default (`__depthmode 1`) un-shadows a *whole* prim when its
  base is in front of the caster. Correct-ish, but the shadow can't *climb* a prim that is genuinely
  behind: it's either fully lit or the ground band cuts across at the wrong height.

## The geometry (converged with the user)
The world is a **65° tilted plane rendered flat** (the oblique art style). One screen row maps to a ground
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

- **ONE shadow map, two disjoint passes** ([forks F3](forks.md#f3), user) — NOT two maps + a composite.
  Pass 1 (ground) writes shadow wherever the surface/`zdepth` map says there's **no prim**, and
  **discards on prim texels** (their ground shadow is invisible under the sprite anyway). Pass 2 (prim)
  writes **only** on prim texels, into the **same** `shadow-cold` (cold/hot, per-light u9). Partitioned by
  presence ⟹ they never collide, and there is **no composite** — the consumer samples the one map by
  world position and gets ground-shadow on ground, prim-shadow on prims automatically.
- **Pass 2** on a prim texel: **lift the receiver to `z`** (fictional height from the depth map) and cast
  via the CONE construction (below) — reusing `casterCover` + the corridor, **excluding same-tile
  casters** (self). The `__depthmode` binary path stays a fallback while this comes up.
- **Order + dirty:** run ground then prim; disjoint writes, **no clear between**. The dirty is the UNION
  (a texel dirty if *either* shadow changed); the owning pass rewrites it, the other discards. A prim
  moving in/out flips a texel's owner — the move already dirties that region, so it re-partitions cleanly.
- **Transparent gaps** read as "no prim" (the bake discards transparent fragments) ⟹ they correctly fall
  to pass 1 and show the ground shadow through the sprite.

## Pass 2 — the CONE construction (ratified 2026-07-23, user)
The per-caster→receiver test, built entirely on the [world-geometry](../2026-07-23-world-geometry/README.md)
model (`z = sin65·(px.y − base)`, `s = z_light/(z_light − z_point)`, x separable). This **supersedes the
re-sample-the-ground-shadow framing** ([`issues.md#i1`](issues.md#i1)) — it computes the billboard shadow
directly, so self-shadow and caster-height are handled by construction:

- **`shadow.tip.y`** — project the caster's TOP (at its fictional height) through the light onto the
  ground; that ground point's screen-y is the shadow's far edge.
- **Cull (which casters hit a receiver):** `shadow.tip.y < receiver.bottom.y < prim.bottom.y` AND x
  inside the cone. Far bound = within the shadow's reach (past the tip → lit); near bound = the receiver
  is BEYOND the caster (caster between it and the light). The near bound is **free**: if the receiver is
  in front of the caster, the shadow throws the other way and would only fall on the back face we never
  render — "in front → no cast" needs no guard.
- **Climb (extent on the receiver):** the caster-top ray crosses the receiver at some height; **below
  that → shadowed, above → lit** (tiny at the tip, full near the caster). No separate top-edge calc.
- **Shape:** within the band, sample the caster's **silhouette** (surface coverage) at the ray∩caster
  point — `v` from the cone, `u` from the x-interpolation (`caster.x = recv.x + s·(light.x − recv.x)`).
  This ray∩silhouette is the **exact** test; the cull is conservative (never misses). Exclude same-tile
  casters (self). Accumulate over casters (max).

## What we already have
`zdepth-world` (base row + is-thing, the coverage/surface G-buffer model — now populated), the caster
**buckets** (prim indices) + light records in the unified data texture, `casterCover` + the corridor
walk, the cold/hot **dirty** machinery, and the binary front/behind fallback (`__depthtest`). The
self-shadowing re-sample shortcut was removed ([`issues.md#i1`](issues.md#i1)). Layouts authoritative in
[`docs/VARIABLES.md`](../../VARIABLES.md).
