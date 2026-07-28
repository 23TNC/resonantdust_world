# Billboard-quad shadows — the projected-silhouette model (design / shape)

The **shape** of a caster's ground shadow: a small fan of textured triangles, the caster's
billboard silhouette projected radially from a point light onto the ground. Converged in the
interactive sandbox [`bin/shadow-projection-sandbox.html`](../../../../../bin/shadow-projection-sandbox.html)
(also a live artifact — `claude.ai/code/artifact/89cfbfe2-2922-4c3d-ac91-123f3e2506ee`). This
supersedes the "wedge" fake still described in [`lighting.md` §D](lighting.md); the *why* is in
[`intent/shadows.md`](../intent/shadows.md).

## Projection

A sprite is a **billboard quad** (width `W`, height `H`). It is tilted off the ground by the ground
angle `θ` — always `R_x(θ)` (mixes world Y and Z, never X) — so its top leans away from vertical.
Each quad point `p = (x,y,z)` casts to the ground plane `z=0` from the light `L = (Lx,Ly,Lz)`:

```
if p.z <= 0:  ground = (p.x, p.y)                    # at/under ground → drop straight down (footprint)
else:         t = min( Lz / (Lz - p.z), TMAX )       # TMAX = 8  (clamp runaway grazing projections)
              ground = L.xy + t·(p.xy - L.xy)
```

Corner order is `[TL, TR, BR, BL]` = uv `[(0,0),(1,0),(1,1),(0,1)]`. `sh = corners.map(project)`.

## Per-facing setup

Facing comes from sprite rotation (0=S, 1=E, 2=N, 3=W; W = E mirrored). Two regimes:

- **E/W** (side-on) — width runs east-west; the quad **roots on its foot edge** (base on `y=0`) and
  leans north. Base corners are forced to the footprint: `BR=(W/2,0,0)`, `BL=(-W/2,0,0)`.
- **N/S** (front/back) — the quad is additionally **rolled `R_y(±90)`** so `worldX = 0` (edge-on),
  and **roots on the centerline**: `BR=(0, +W/2, 0)`, `BL=(0, -W/2, 0)`. Roll sign +1 for S, −1 for N.

`BC` = midpoint of the two base corners.

## The 5-triangle fan + per-corner depth

Each base corner spawns a `+depth` and `−depth` variant **without moving the corner itself**; two
depths per facing — `dA` drives the `BL` corner, `dB` drives `BR`:

| triangle | verts | note |
|---|---|---|
| **T1** | `TL, TR, BC` | body |
| **T2** | `TL, BL⁺, BC` | `+depth` |
| **T3** | `TR, BR⁺, BC` | `+depth` |
| **T4** | `TL, BL⁻, BC` | `−depth` |
| **T5** | `TR, BR⁻, BC` | `−depth` |

- **E/W** offsets the variant on **y**, but its `+` variant is **+0** (`BL⁺=BL`): the `+` edge seats on
  the billboard foot, and only the `−` variant spreads north. `dA`=left, `dB`=right.
- **N/S** offsets the variant on **x** (symmetric ±). `dA`=top, `dB`=bottom (the N-S base ends).

## UV sampling (the shader path)

The shadow is drawn as `(position, uv)` triangles sampling the sprite; the sandbox proves it with an
affine per-triangle texture map, which is exactly what a fragment shader computes across a flat
(already-projected) triangle. Sample the sprite **alpha as the shadow mask** — the UVs below are what
port to the game; the sandbox's red/blue/oval test textures are only a coverage check.

- **E/W** — sample the **bottom edge**: `TL(0,0) TR(1,0) BL±(0,1) BR±(1,1) BC(0.5,1)`.
- **N/S** — base points sample the sprite's **vertical centerline** (`u=0.5`, `v` spanning the base);
  tip points (`TL/TR`) sample the sprite **edge nearest the light** (`u=0` when the light is left of
  the caster, `u=1` when right). So an N/S shadow reads from one texture half.

## Depth from presence (auto rule; DSL-overridable)

`dA`/`dB` are **computed from the sprite**, not authored (override in the DSL when art needs it).
Build a **presence map** once per sprite-facing (alpha thresholded into a bitmap — measure geometry,
never per-frame alpha). Working **in from both edges** to the first present pixel gives a row's opaque
width / a column's opaque height. Each depth = **½ · average opaque extent over its half**, scaled
texture→sprite:

- **E/W**: `dA` = ½·avg opaque **height** of the **left**-half columns; `dB` = ½·avg of the **right** half.
- **N/S**: `dA` = ½·avg opaque **width** of the **top**-half rows; `dB` = ½·avg of the **bottom** half.

**Both scale by `H`** — the depth-bearing axis spans the quad *height* in both facings: E/W's
north-south lean is `H` tall, and the rolled N/S footprint's east-west width equals the quad height.
So `depth = ½ · (avgExtent_texpx / TS) · H`. (This is a bake step — see the cost note in the intent.)

## Porting to the game

Hand the shadow pass the same 5 `(position, uv)` triangles per caster and sample the sprite alpha as
a shadow mask. The projected silhouette is the primitive the [tiered lighting](../intent/tiered-lighting.md)
consumes — the cold inline sweep tests it per fragment, the dynamic tier rasterizes it. **Known code
gap:** `shadowPass.ts`'s `nsProject` currently uses a `θ`-on-X/Z axis, *not* this model's
`R_y(±90)·R_x(θ)` — reconcile before wiring these UVs in.
