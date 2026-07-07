# Lighting — current state & design

The renderer's normal-mapped deferred lighting system: what's built, how it maps onto our
non-standard projection, and where it should go. Sibling to `docs/art-style.md` (projection),
`docs/de-lighting.md` (albedo derivation), `docs/texture-paths.md` (map layout).

## Where we are (implemented + verified)

A **screen-space deferred** model. `SquareCache` bakes the visible world into one fixed-size
toroidal composite **per channel** — `albedo`, `normal`, `surface`, `depth` — from a shared prim
index. The display mesh draws those composites through `LightingShader` (`pixijs/src/game/
viewport/lightingShader.ts`), which lights every fragment in one pass. The composites *are* the
G-buffer; there are no per-object light passes.

### The coverage model (one silhouette source of truth)

Every masked channel keys its coverage off **one** place — the **surface map's B channel** — so
silhouettes can't drift between channels (the class of bug that caused halos/speckles/ghost
shadows before). The surface map is RGB data: **R = height** (reserved; unused by the current
wedge-shadow lighting), **G = ambient occlusion**, **B = coverage/transparency** (the diffuse
alpha, straight — 0 outside the object). It carries **no stored alpha**: data in a source alpha
channel fights the client's upload-premultiply and the gate's on-demand downscale, so coverage is
*derived* at bake time and premultiplied into each composite. Two derivations:

- **Soft α** (`surface.B` directly) premultiplies the **albedo** and **normal** bakes — the
  visual transparency. A thing's edge blends smoothly toward whatever's behind it.
- **Hard presence** (`smoothstep(surface.B)`) premultiplies the **depth** and **surface** bakes
  — a semi-transparent pixel still *fully* occupies its cell for occlusion, so a thing's edge
  never reads as ground (no halo). The lighting pass recovers everything `/A`: `ao = surface.G/A`,
  presence = `surface.A`, receiver depth = `depth.B/depth.A`, opacity = `albedo.A`.

The shadow caster silhouette, the `originFrac` trunk-base probe, and the presence gate all read
`surface.B` — nothing reads albedo-alpha anymore (the albedo map is the RGB residual base).

### Built (client verified in-browser; the surface-map pipeline needs an asset regen to test)

- **A — map-aware texture path.** Server (`server/src/textures.rs`, `tex_manifest.rs`, `main.rs`)
  serves any map in `MAPS` (`albedo|normal|layers|surface|depth|emissive`) via
  `/textures/lod/<hash>/<size>/<map>/<stem>`; the manifest advertises which each stem has. Client
  (`TextureResolver`, `lod.ts`, …) resolves per `(stem, map)`; a stem lacking a map stays on that
  channel's flat fallback and never fetches. The data maps (`albedo`, `layers`, `normal`,
  `surface`) load **straight** (no premultiply) so their RGB survives intact.
- **B — ambient + directional sun.** `outColor = albedo · (ambient + sun·halfLambert(N·sunDir))`.
  Half-Lambert wrap (0.4). Normal-mapped tiles show relief.
- **C — dynamic point lights.** `LightRig` holds the sun, ambient, ≤32 point lights (world px,
  `height`/`radius`/`color`/`brightness`/`castsShadow`), and a cursor light packed each frame.
  Quadratic falloff, half-Lambert. The cursor light tracks real pointer input with world-space
  falloff.
- **D — wedge shadows + presence occlusion.** `ShadowPass` (`shadowPass.ts`) rasterizes each
  standing caster's billboard "wedge" (its `surface.B` silhouette, projected radially to the
  light) into a screen-space coverage RT carrying the caster's **tile-depth** (a z-order ring, 1
  step per 5 world px — *not* a height field). The lighting pass compares that against the
  receiver's own tile-depth (the `depth` composite) with a wrapped difference: where the receiver
  sits at/in front of the caster the shadow is behind it, so only the see-through fraction
  `sh.a·(1−opacity)` shows through (opaque → full light, glass → the shadowed background reveals).
  See `docs/shadow-design.md` for the wedge geometry.
- **E — ambient occlusion (surface.G).** AO is derived offline (`art ao`, `bin/lib/depth_ao.py`
  integrates the untilted normal → height → HBAO horizon march → `occlusion.png`) and packed into
  **surface.G** (was normal.α). The lighting pass reads `ao = surface.G / surface.A`: ambient
  gated fully (`ambient·ao`), direct light partially (`mix(1, ao, AO_DIRECT)`), so crevices read
  as depth, not flat plastic. Self-occlusion only (intra-sprite).

### Coordinate note (a fixed bug worth remembering)
The display mesh's `aPosition` is the **panned** world coord (`worldX + panX`, from
`SquareCache.fillDisplay`), so the shader's `vWorld` is panned-world, while `LightRig` positions
lights in **pure** world px. The shader reconciles them with a `uPan` uniform: `toL = ld.xy + uPan
− vWorld`. The two pan terms cancel for a world-pinned light (stays put under pan) while the
cursor light — re-derived via `screenToWorld` each move — tracks the pointer.

### Not yet built
Emissive (channel reserved, not sampled), day/night cycle, billboard-aware object normals,
object/ground-aware shadows, cold/static light baking, real depth-map art.

## The projection, and why it dictates the model

Per `docs/art-style.md`: **the world grid is viewed top-down, but objects/characters are drawn in
oblique 3/4 perspective** (they show fronts/sides; E/W = side, S/N = front/back). This is the
RimWorld/Prison-Architect model, and it splits lighting into **two regimes**:

- **Ground / floor / terrain — a true top-down height field.** The grid is square and top-down, so
  **screen XY = world XY exactly** (no foreshortening; `SQUARE`=64 px both axes) and a fragment's
  pseudo-world position is `vec3(worldXY, depth·heightScale)` where **depth = height above the
  ground plane, not camera distance**. Normals are in the ground frame: **+Z (blue) = up**, ±X/±Y =
  map east/north. Our current shader is *correct* for this regime.
- **Standing objects — oblique billboards.** A tree/pawn sprite is a vertical card facing the
  camera, baked into the composite at its **screen** footprint (extending up-screen for height).
  Two things break here: (1) its Laigter normal's +Z points **out of the card toward the camera**,
  not up from the ground; (2) its up-screen pixels conflate "higher" and "further north." So the
  ground-frame assumptions are wrong for object pixels — they need billboard-aware handling.

The single most important rule (and the one a naïve 3D/isometric port gets wrong): **the depth
texture is height above the map plane.** The oblique orthographic view has no consistent
camera-depth axis; treating depth as camera distance would light everything wrong.

## Direction (the load-bearing constraint)

**This is a dense many-lights world, not a sun-lit one.** The common light source is **point
lights authored on DSL primitives** (`^light`-style defs): expect **many static lights** + a
**fair number of dynamic (mover) lights** at once. Darkness + light pools is the aesthetic —
**ambient and a directional sun are at most a faint floor, never the model**, and a day/night
cycle is not a priority. So the target architecture is the old game's **tiered cold/warm/hot**
system — bake static-light contributions into a lightmap, keep dynamics in a live pool, solve
shadows at that scale — NOT the ≤32-uniform forward pass built for the 0.1 MVP. The MVP is a
starting point that must grow into this. (See `../resonantdust/view/src/game/lighting` +
`docs/tiered_lighting.md` there.)

## Design directions (see the session discussion for trade-offs)

- **Tiered static-light baking (the headline).** Bake cold/static lights (the DSL-authored bulk)
  into a per-region lightmap so hundreds cost one texture sample; keep dynamic/mover lights in the
  live uniform pool; shadow via scatter maps / an occlusion bitfield. This is the scalability
  spine, not a "later" item — the light count demands it from the start. Wire DSL `^light` prims
  into `LightRig` as first-class registrants.

- **Two-regime shader.** Tag object pixels in the composite (a flag bit, e.g. via the depth blue
  band as the old game did, or a dedicated channel) and branch: ground pixels use the current
  ground-frame math; object pixels rotate their tangent-space normal into the world frame **by
  facing** (a fixed per-facing rotation, S/N/E/W) and don't self-shadow as if they were terrain.
  This is the principled version of the old game's `SOUTH_TILT` hack.
- **Fragment height in the light vector.** Cheap ground refinement: `toL.z = light.z − fragHeight`
  (we already sample `fragHeight`). Exact for ground; apply with care to billboards.
- **Emissive.** We have the channel — add `+ emissive` **after** lighting so it glows in darkness
  (lamps, windows, magma). Low effort, high impact.
- **Object shadows.** The height-ray march captures ground self-shadowing, but a billboard's ground
  shadow is a *projected footprint*, not a height-field feature. Options: cheap stylized **blob
  shadows** (a soft dark ellipse per object, offset away from the key light) vs. silhouette
  extrusion. Decide per how much shadow fidelity the art wants. With many lights, shadow cost is the
  scaling worry — the old game's round-robin scatter/bitfield approach is the reference.
- **Day/night cycle.** *Not* a priority (undecided). If ever added, it's a slow modulation of the
  faint ambient floor, not the primary light source.
- **Colour space.** The art is deliberately flat/cel (`art-style.md`), so full linear + tonemap may
  fight the look. Keep perceptual-space multiply for now; revisit if banding/among-lights blending
  needs it.

## Material variation (albedo-side, upstream of light)

Flat de-lit albedos read as plastic. The **material system** adds colour *richness* that reads
as pigment, not light, by exploiting one perceptual fact: **lightness variation reads as light;
hue/chroma variation reads as material.** So it perturbs hue/chroma at (roughly) constant
lightness — in OKLab, holding L fixed — driven by tiling *noise fields*, and it does this
entirely **upstream of, and blind to, the lighting engine**. The lighting pass lights the varied
albedo exactly as it lit the flat one; the two never fight because material owns hue/chroma and
lighting owns value.

**Data flow.** `bin/art split_layers` decomposes the de-lit albedo into a `layers` RGB map (each
channel = a material's per-pixel weight) + a **residual that becomes the `albedo` map** (the RGB
render base); the de-lit source is preserved as the build-only `albedo_marigold.png`. Both
`albedo` and `layers` are RGB — no alpha, so the client loads them straight and the reconstruction
sum stays exact; the visual alpha lives in `surface.B`. A `<material>` DSL registry
(`content/material/*.rd`, mirrored over `/content`) names reusable materials — `{ noiseField,
hueSwing (deg), chromaSwing, warmCoolBias, sampleSpace }`. A prim's `:visual @on_create` binds each
layer channel to a material + a base **tint** (`&thing.packed.0.material` / `.tint` — the DSL
binding kept its `packed` name) → `VisualParts` → the wasm per-def tables (`tilePackedChannels` /
`materialSwings` / …) → the client `MaterialRegistry`, attached to each prim by `def_id`.

**Where the math runs: BAKE time, not the display shader.** This is a deferred/composite
renderer — a display-space fragment can belong to any prim, so per-prim material params can't be
display-shader uniforms. Instead the **albedo composite's per-prim bake** (`SquareCache`) draws a
real-tier prim through `materialBakeShader.ts` (a Pixi high-shader Mesh) — the *single* real-tier
albedo path. It reconstructs, per fragment, in the **delta form**, then applies alpha LAST:

    out.rgb = albedo(residual) + Σ layersᵢ · (jitter(tintᵢ, noise, paramsᵢ) − tintᵢ)
    out.a   = surface.B     (visual alpha, applied AFTER the sum — never premultiplied into it)

The delta form is **identity-safe**: no `layers` map (or a zero-swing material / unbound channel,
`tint 0`) contributes exactly `0`, so a stem with no materials — or the whole world before any is
authored — bakes its albedo (== residual) unchanged. `jitter()` (`oklab.ts`) converts the tint to
OKLab, rotates hue by `hueSwing·noise` (leaned by `warmCoolBias`) and shifts chroma by
`chromaSwing·noise`, **never touching L**, then back to sRGB. Noise is a runtime-built tiling atlas
(`noiseAtlas.ts`; rows = fields, R/G = two decorrelated fields) in **UV space** (rides the sprite)
or **world space** (pinned to the ground).

Assumptions / current limits: the real-tier bake engages once a stem's `albedo` **and** `surface`
LODs load (so it degrades cleanly to the flat geo box on the geo/preview tiers); `layers` is
capped at **3 channels** (RGB — the 4th/alpha material channel was dropped, since data in an alpha
channel fights the upload/downscale path); and the noise atlas is procedural for now
(`bin/lib/noise_fields.py` can bake richer fields into the same row layout). World-space noise is
per-fragment exact for ground tiles; billboard things use UV space.
