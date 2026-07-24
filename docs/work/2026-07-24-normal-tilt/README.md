# Normal-tilt correctness — pitch standing-object normals 25° down into the ground plane — 2026-07-24

_Component: [`bin/art`](../../../bin/art) (bake) + [`client/webgl`](../../components/client/) `game/viewport/`
(consume). Builds on the ratified [`world-geometry`](../2026-07-23-world-geometry/README.md) `WORLD_TILT = 65°`
model and the [`lighting`](../2026-07-23-lighting/README.md) relief pass. Decisions in [`forks.md`](forks.md);
steps in [`todo.md`](todo.md)._

## The model (user, authoritative)
Billboards are drawn **facing the viewer** (parallel to the screen); Laigter generates their normals in that
frame, so a flat normal shoots **out of the screen into the viewer's eyes** (+Z). But **conceptually a
standing billboard is perpendicular to the world ground** — it stands up out of the ground like a post. So
its *surface* is ⊥ the ground and its **normal lies IN the ground plane**, pointing toward the viewer and
down. Lighting is cast in **world space**, so the eyeball-facing normal is wrong and must be re-seated.

## The number: 25° down = 90° − WORLD_TILT
The out-of-screen normal is ⊥ the **screen**. The screen is `WORLD_TILT = 65°` from the ground plane, so the
screen-normal sits **90° − 65° = 25°** off the ground plane. **Pitch the normal 25° down** (toward
screen-bottom) and it seats into the ground plane — "from the monitor into the floor." The pitch is the
**complement** of the world tilt, so it is keyed to `WORLD_TILT_DEG`, not a fresh constant.

Flat `(0,0,1)` → pitch 25° down → `(0, −sin25, cos25) = (0, −0.423, 0.906)`. Gentle south/viewer-facing
horizontal bias (`sin25 ≈ 0.42`), Laigter relief riding on top undistorted.

## The bake (rigid rotation about the E–W / screen-x axis)
```
θ = --pitch_normal degrees      # geometrically-correct = 25 (= 90 − WORLD_TILT)
ny' = ny·cosθ − nz·sinθ
nz' = ny·sinθ + nz·cosθ         # nx unchanged; renormalize
```
Keep the runtime `uNormalYSign(-1)` convention (the downward pitch already yields negative green in-frame).

**The pitch is an explicit knob, `--pitch_normal DEG`, defaulting to `0` (no change).** 25° is the
geometrically-correct value (the complement of `WORLD_TILT_DEG=65`, kept as reference constant `NORMAL_PITCH_DEG`
+ echoed in help), but it is NOT auto-applied — the operator passes `--pitch_normal 25` to bake it, so the
value can be tuned in-browser without editing the script. Skipped at 0 or for grid tiles (`_resolve_tilt` OFF).

## What's wrong today
`_normal_tilt` ([`bin/art:767`](../../../bin/art)) does an **additive bias + Z-squash** — `ny -= 1.8;
nz *= 0.18; renormalize` (defaults `tilt_amount=1.8`, `tilt_z=0.18`, inherited from the old game's
`SOUTH_TILT` hack). Two faults: it lands the flat normal at **~84°** (over-pitched — nearly flat in the
ground plane, way past 25°), and being an additive bias it **distorts** the Laigter relief instead of
rigidly rotating it. Ground tiles correctly get **no** pitch ([`bin/art:824`](../../../bin/art)
`_resolve_tilt` OFF for grid) — their normals are already ground-frame; only standing billboards need the 25°.

## Not blocked on the generator
Laigter has **no world-tilt input** — it always emits tangent-space (card-plane) normals. The pitch is a
deterministic post-pass — exactly what `_normal_tilt` already is, with the wrong math. Feed the angle
(`90 − WORLD_TILT`) to *that* step; every generated normal is corrected automatically, no hand-authoring.
Marigold view-space normals need a *different* rotation and are not wired yet ([`bin/art:2341`](../../../bin/art)).

## Scope
Replace the `_normal_tilt` bias with the 25° rotation; drive `θ = 90 − WORLD_TILT_DEG` from one shared
constant (same source as [`shadowGather.ts:52`](../../../client/webgl/src/game/viewport/shadowGather.ts));
retire `tilt_amount`/`tilt_z`. Re-bake the object corpus; verify relief direction in-browser.
