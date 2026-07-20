# Completed — light-data-texture

_Done + verified. Items move here from [`todo.md`](todo.md)._

---

## L1 · Float data texture — 2026-07-20

`ShadowCast` holds a `Float32Array` and a `Texture` over a `BufferImageSource`
(`format: "rgba32float"`, `scaleMode: "nearest"`), built in the constructor. Pixi 8.9.1 maps
`rgba32float` → `gl.RGBA32F` internal + `gl.FLOAT` type (WebGL2 core) — verified, no extension needed.
Column x = light index. **Compacted to 5×2** (2 rows/light) once the f32 precision was proven: row 0 =
`world_x, world_y, world_z, radius`, row 1 = the RGBA colour — no hierarchical position split, intensity
folded into the colour. (Started 5×5 with the u8-parity 5-row layout; see the README field table.)

## L2 · Fill + re-roll colour once/sec — 2026-07-20

`fillLightData(rollColors)` packs each light's 2-texel column each tick — row 0 world_x/_y/_z + radius as
real world-px floats — then `source.update()` re-uploads. **Row 1 (colour) is re-rolled once/sec**
(`COLOR_INTERVAL_MS`), held steady between rolls; `lastColorMs` starts in the past so frame 1 initialises.
The colour is mirrored into `curColors` so the debug markers draw the same colour the shader reads.

## L3 · Display reads colour from the texture — 2026-07-20

`ShadowTDisplayShader` gained a `uLightData` sampler and a `decodeBitsTex(n)` that, for each set bit `k`,
samples column `k` row 1 at the texel centre `((k+0.5)/5, 1.5/2)` and accumulates it — replacing the
hardcoded `LIGHT_COLORS`/`DECODE` palette (constant removed). Colour now comes **only** from the texture.

## L4 · Verified in-browser — 2026-07-20

`?focus=100,50&shadowcast`: each light's shadow **changes colour once per second, independently per
light** (holds steady between rolls — verified back-to-back frames identical, a frame ~1s later changed),
and **each marker dot matches its shadow's colour** — the shader-sampled texel equals the JS value written.
The full path (JS → float texture → `update()` → shader sample) is live. Panning far still shows no ghosts
(the `shadow-tiered` merge/invalidation is untouched). Console clean — no incomplete-texture/format errors,
so the float path worked and the u8 fallback ([F1](forks.md#f1)) was not needed.

---

**Deferred (optional):** L5 — sourcing light *position*/radius from the texture in the cast/markers
(instead of the JS `lights[]`) is a genuine stretch, not needed to prove the data path. It graduates with
the real [`shadows`](../shadows/README.md) engine, where the cast becomes a shader that reads the texture.
