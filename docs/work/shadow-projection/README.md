# Work — shadow-projection (align the cast with the 5-triangle projected-silhouette model)

_Opened 2026-07-21. Bring the webgl `ShadowCaster` from its basic-cut **analytic trapezoid** onto the
**projected-silhouette 5-triangle** model that converged in the sandbox — the actual triangle geometry the
game needs. Component: `client/webgl` (docs in [`webgl-engine`](../webgl-engine/README.md)). Authoritative
spec: [`design/shadows.md`](../../components/client/webgl/design/shadows.md); interactive proof:
[`bin/shadow-projection-sandbox.html`](../../../bin/shadow-projection-sandbox.html) (live artifact
`claude.ai/code/artifact/89cfbfe2-2922-4c3d-ac91-123f3e2506ee`). Intent:
[`intent/shadows.md`](../../components/client/webgl/intent/shadows.md)._

## Why — the current cast is a placeholder shape

[webgl-engine W4f](../webgl-engine/completed.md) shipped the first lights + shadows as a **basic cut**
([D-2](../webgl-engine/deviations.md#d-2)): `shadowCaster.ts` casts, per fragment, a **single trapezoid** per
(light, caster) — the billboard's bottom edge projected radially by **one** factor (`Lz/(Lz−H)`, one height
`H` for the whole quad), tested point-in-quad in a fullscreen loop. It reads as a flat fan; it is **not** the
model the game wants. The sandbox converged the real shape, and this stream ports it.

## The target shape (from the sandbox / `design/shadows.md`)

A caster's ground shadow is a small **fan of triangles** — the caster's **billboard silhouette** projected
radially from the point light onto the ground:

1. **Billboard quad, tilted by the ground angle `θ`** (`R_x(θ)` — mixes world Y & Z, never X), so the top
   leans away from vertical. Corners `[TL, TR, BR, BL]` = uv `[(0,0),(1,0),(1,1),(0,1)]`.
2. **Per-corner radial projection** — each corner projects to `z=0` by **its own z**:
   `t = min(Lz/(Lz−p.z), TMAX=8)`, `ground = L.xy + t·(p.xy − L.xy)`; a sub-ground point drops straight down
   (the footprint). (Today's cast uses ONE factor for the whole quad — the key delta.)
3. **Base corners rooted to the footprint** — `BR/BL` forced onto the ground edge (`z=0`).
4. **The 5-triangle fan** with a base-centre `BC` and a **±depth** spread per base corner
   (`dA`→BL, `dB`→BR): **T1** body `(TL,TR,BC)`; **T2/T3** `+depth` `(TL,BL⁺,BC)/(TR,BR⁺,BC)`; **T4/T5**
   `−depth` `(TL,BL⁻,BC)/(TR,BR⁻,BC)`. The depth gives the shadow **base a width matching the sprite's
   silhouette** (a round tree casts a rounded base, not a thin line) instead of a zero-thickness edge.
5. **Two regimes by facing** — **E/W** (side-on) roots on the foot edge + leans north (`+`variant seats at
   `+0`); **N/S** (front/back) is rolled `R_y(±90)` (edge-on) + roots on the centerline (symmetric ±). Facing
   comes from the sprite rotation (0=S,1=E,2=N,3=W; W = E mirrored) — the SAME rotation that picks the sprite.
6. **Alpha-masked triangles** — drawn as `(position, uv)` triangles sampling the sprite **alpha as the shadow
   mask** (E/W bottom-edge UVs; N/S centerline + light-nearest-edge UVs), so the shadow is silhouette-shaped.
   The sandbox proves it with an affine per-triangle texture map — exactly what a fragment shader does over a
   flat, already-projected triangle, so it **ports 1:1**.
7. **Depth from presence** — `dA/dB` are **computed from the sprite** (½·avg opaque extent over each half,
   scaled ×`H`), from a per-facing presence map (the silhouette). W4h now gives us that silhouette
   (`surface.B` coverage), so this is unblocked. DSL-overridable when art needs it.

## The strategy shift — same geometry, built on the GPU

**Analytic per-pixel → GPU-built geometry.** Today the cast is a fullscreen fragment that loops casters and
tests point-in-trapezoid. The target is the 5-tri fan — but built **on the GPU, not the CPU**. The sandbox
(and the earlier draft of this plan) build the fan on the CPU: JS computes the projected corners + depth and
uploads a vertex buffer. Instead, we upload only the **raw caster + light data** and let an **instanced vertex
shader** do the work — one instance per (light, caster) pair, `drawArraysInstanced(TRIANGLES, 0, 15, N)`; the
vertex shader reads the instance's caster (`x,y,W,H,θ,facing,dA,dB` + atlas frame) and light (`Lx,Ly,Lz`, bit),
runs `cornersWith` + `proj` (§Projection above) to place each of the 15 fan vertices, and the fragment samples
the sprite alpha as the mask. **No per-frame CPU geometry rebuild.**

This is the design's intended path AND the migration's payoff — it **delivers `caster-lut` C5 / [webgl-engine
W7](../webgl-engine/todo.md)** (the instanced VTF cast) on the owned engine, where Pixi's walls blocked it. The
CPU keeps only the **cheap parts**: the light→caster in-range **pairing** (the `caster-lut` LUT — index work,
not geometry) and the **once-per-sprite presence bake** for `dA/dB` (not per frame). Everything per-frame and
per-vertex is GPU. The fan is exactly the **`shadow-hot` generator** the [`shadows`](../shadows/README.md)
stream's screen-hot→world-cold bitfield consumes; folding it into the persistent bitfield + round-robin stays
in that stream ([webgl-engine W4f remainder](../webgl-engine/todo.md)).

## Scope

In: the GLSL projection primitives (vertex-shader `cornersWith`/`proj`), the **GPU-instanced** 5-tri fan, the
caster+light data channel (instance attrs / data texture), depth-from-presence (a one-time CPU bake feeding GPU
data), the two-regime facing, alpha-masked triangles — the instanced draw replacing the analytic loop. Out: the
world-cold bitfield persistence + round-robin (the `shadows` stream), real per-light lighting (the shadow stays
a default-on debug overlay per [D-2](../webgl-engine/deviations.md#d-2)), DSL depth/θ authoring (auto first).

## Open decisions

See [`forks.md`](forks.md): **F1** caster facing source (prims carry `flipX`/`cell`, not rotation); **F2**
solid tris first vs alpha-masked from the start; **F3** the GPU **data channel** — per-instance vertex attrs
vs a `caster-lut` data texture (VTF/`texelFetch`) — and where the fan rasterizes (swappable target for the
`shadows` bitfield); **F4** where `θ` + the depth bake live (art-style constant / per-def / DSL).
