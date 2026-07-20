# Completed — light-data-texture

_Done + verified. Items move here from [`todo.md`](todo.md)._

---

## L1 · 5×5 float data texture — 2026-07-20

`ShadowCast` holds a `Float32Array(5·5·4)` and a `Texture` over a `BufferImageSource`
(`format: "rgba32float"`, `scaleMode: "nearest"`), built in the constructor. Pixi 8.9.1 maps
`rgba32float` → `gl.RGBA32F` internal + `gl.FLOAT` type (WebGL2 core) — verified, no extension needed.
Column x = light index, rows y = the 5 data pixels.

## L2 · Fill + randomise every frame — 2026-07-20

`fillLightData()` packs each light's column each tick — anchor_x/_y/_z + radius + intensity as real
world-px floats (region/zone/tile left 0; float mode stores world-px directly), and **row 3 = a fresh
random bright RGB** — then `source.update()` re-uploads. The colour is mirrored into `curColors` so the
debug markers draw the same colour the shader reads.

## L3 · Display reads colour from the texture — 2026-07-20

`ShadowTDisplayShader` gained a `uLightData` sampler and a `decodeBitsTex(n)` that, for each set bit `k`,
samples column `k` row 3 at the texel centre `((k+0.5)/5, 3.5/5)` and accumulates it — replacing the
hardcoded `LIGHT_COLORS`/`DECODE` palette (constant removed). Colour now comes **only** from the texture.

## L4 · Verified in-browser — 2026-07-20

`?focus=100,50&shadowcast`: consecutive frames show each light's shadow **changing colour every frame,
independently per light** (magenta→peach, blue→indigo, …), and **each marker dot matches its shadow's
colour** — the shader-sampled texel equals the JS value written that frame. The full path (JS → float
texture → `update()` → shader sample) is live. Panning far still shows no ghosts (the `shadow-tiered`
merge/invalidation is untouched). Console clean — no incomplete-texture/format errors, so the float path
worked and the u8 fallback ([F1](forks.md#f1)) was not needed.

---

**Deferred (optional):** L5 — sourcing light *position*/radius from the texture in the cast/markers
(instead of the JS `lights[]`) is a genuine stretch, not needed to prove the data path. It graduates with
the real [`shadows`](../shadows/README.md) engine, where the cast becomes a shader that reads the texture.
