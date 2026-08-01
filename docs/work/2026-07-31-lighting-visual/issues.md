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
