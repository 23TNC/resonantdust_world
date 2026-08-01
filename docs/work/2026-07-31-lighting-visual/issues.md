# Issues — lighting visual correctness

## I1 — the shadow-base offset, pinned per kind {#i1}

Probed live (cold load, records vs drawn boxes vs bbox fractions), in UNITS:

| stem | span | (a) anchor error: record C.y − drawn opaque bottom | (b) height-window bottom above C |
|---|---|---|---|
| conifer/e | 2 | **4.5** | 4 |
| flora/e | 1 | 1.9 | 2 |
| wolf/e | 1 | **4.5** | 5 |
| human head (11/e.1) | 1 | 4.4 | 5 |
| human body (7/e) | 1 | 1.1 | 1 |

The two columns are the SAME letterbox margin counted twice: (a) the record's world anchor
`C` is the FRAME box bottom, which sits below the drawn art's opaque bottom by the art's
bottom margin; (b) the height window `[fu − subY − subH, fu − subY]` starts that same
margin above `C`. Vertically they cancel (the caster's base height ≈ the drawn feet), but
the card's PLAN line is `C.y` — so every shadow's origin line sits the margin SOUTH of the
visual feet (~half a tile for the conifer/wolf), the visible detachment. The user's model
(bbox bottom-aligned to the anchor: window `[0, subH]`, `C` = drawn opaque bottom) zeroes
both columns by construction — P1's re-probe is exactly this table at 0.

West-flip and wolf-n rows were not live in the probed window; the mechanism is
facing-independent (the margin comes from the letterboxed master), and P1's re-probe
covers whatever faces exist at fix time.

## I2 — `__lightexact` reports glError 1282 (pre-existing, drill-path only) {#i2}

The drill's baseline `lights.run(...)` is called WITHOUT opts, so every optional sampler
falls back to the uint `prim` texture — including `sampler2D` uniforms (`uSurfaceAtlas`
before this stream, the P2 normal samplers now). Binding an integer texture where the
program declares a float sampler raises `INVALID_OPERATION` at draw time. This is
debug-drill-only (the real draw loop always passes real textures), predates P2, and does
not affect the drill's verdict (`bitIdentical` computed from readPixels diffs, which
succeed). Fix if it starts mattering: give `LightPass` a 1×1 white placeholder for the
float samplers instead of `prim`.

## I3 — "tiles have no working normals": terrain is GEO-TIER, by construction {#i3}

Probed live: the ONLY textured ground stem in the window is `biome-tile/default/smooth/wall/l`
(normal resolves, real per-cell normals confirmed IN `normal-cold` and shading on screen).
Grass/dirt/sand terrain prims carry NO textureName — they are geoColor solid fills, so their
baked normal is the flat-up constant (the normal resolve hook's `tint 0x8080ff` path). That is
the documented state of terrain until [`2026-07-29-texture-generalization`](../2026-07-29-texture-generalization/README.md)
(open, user-authored) gives tiles linked textures — grass DOES have `normal.l.0.png` mastered
and manifest-listed, waiting. Not a lighting defect; the sampler consumes whatever the bake
holds the moment terrain gets real maps.

## I4 — the lod-1 "black wall surface" does not reproduce post-torus {#i4}

The zoom-0.5 `surface-cold` overlay showed wall tiles at (0,0,0) BEFORE the torus rework; after
it, the same wall tile probes healthy at lod 1 (`surf (255,253,255)`, real normal). Most likely
the overlay caught unbaked slots (the scratch clears black) during the pre-fix partition churn.
Watch for it in later sweeps; nothing to fix now.
