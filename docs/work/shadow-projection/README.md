# Work — shadow-projection (align the cast with the 5-triangle projected-silhouette model)

_Opened 2026-07-21. Bring the webgl `ShadowCaster` from its basic-cut **analytic trapezoid** onto the
**projected-silhouette 5-triangle** model that converged in the sandbox — the actual triangle geometry the
game needs. Component: `client/webgl` (docs in [`webgl-engine`](../webgl-engine/README.md)). Authoritative
spec: [`design/shadows.md`](../../components/client/pixijs/design/shadows.md); interactive proof:
[`bin/shadow-projection-sandbox.html`](../../../bin/shadow-projection-sandbox.html) (live artifact
`claude.ai/code/artifact/89cfbfe2-2922-4c3d-ac91-123f3e2506ee`). Intent:
[`intent/shadows.md`](../../components/client/pixijs/intent/shadows.md)._

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

## The strategy shift

**Analytic per-pixel → geometry.** Today the cast is a fullscreen fragment that loops casters and tests
point-in-trapezoid. The model is **rasterized triangle geometry**: build the 5-tri fan per (light, caster) and
draw it (into the screen-space shadow field). That's cheaper (only shadowed pixels are touched, by
rasterization), gives the shaped/alpha-masked shadow, and is the design's intended path — the triangles are
exactly the **`shadow-hot` generator** the [`shadows`](../shadows/README.md) stream's screen-hot→world-cold
bitfield consumes. This stream produces the triangles; folding them into the persistent bitfield + round-robin
stays in that stream ([webgl-engine W4f remainder](../webgl-engine/todo.md)).

## Scope

In: the projection primitives, the 5-tri fan, depth-from-presence, the two-regime facing, alpha-masked
triangles, rasterizing them into the shadow field (replacing the analytic loop). Out: the world-cold bitfield
persistence + round-robin (the `shadows` stream), real per-light lighting (the shadow is still a debug overlay,
default-on per [D-2](../webgl-engine/deviations.md#d-2)), DSL depth/θ authoring (auto from presence first).

## Open decisions

See [`forks.md`](forks.md): **F1** caster facing source (prims carry `flipX`/`cell`, not rotation); **F2**
solid tris first vs alpha-masked from the start; **F3** geometry batching + how it feeds the shadow field;
**F4** where `θ` + the depth bake live (art-style constant / per-def / DSL).
