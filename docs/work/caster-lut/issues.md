# Issues — caster-lut

_Gotchas to respect while building. The structure is standard (CSR indirection); these are the stack-
specific sharp edges._

---

## I-1 · Indices are `f32`, exact only below 2²⁴ — fine here

ES 1.00 has no integer textures ([`light-data-texture` I-1](../light-data-texture/issues.md)), so the LUT
and `start_index`/`count` are `RGBA32F` floats. A float32 represents integers **exactly up to 2²⁴
(16,777,216)**; all indices (caster ≤ 4,096; LUT ≤ 49,152) are far under, so `floor(texel + 0.5)` recovers
them losslessly. Never let an index field exceed 2²⁴.

## I-2 · `texSubImage2D` bypasses Pixi's whole-resource upload

Pixi's `BufferImageSource.update()` re-uploads the entire `Float32Array`. A true partial upload (patch one
caster's 3 px, or only the used LUT prefix) means calling `gl.texSubImage2D` on the underlying `GLTexture`
directly — reach it through the renderer's texture system (`renderer.texture.getGlSource(source)` or the
GL backend's texture map) and pass the sub-rect + a `Float32Array` view. Straightforward in WebGL2 for
`RGBA32F`; just manual GL, outside Pixi's abstraction.

## I-3 · Caster-index stability — or rebuild the LUT with the prim texture

The LUT stores caster-def **indices**. If the prim texture compacts (a caster removed → later defs slide
down), every LUT entry past it silently points at the wrong caster. Either allocate def slots from a
**stable free-list** (removal leaves a hole, reused later) or **rebuild the LUT whenever def indices shift**.
At experiment scale, rebuild-all when the caster set changes is trivial.

## I-4 · LUT overflow is the scaling ceiling — log it

Total LUT usage = `Σ count` over lights (per-light runs, [F2](forks.md#f2)), so heavy overlap of many
large-radius lights hits `49,152` first — well before the 4,096 caster or light slots. If a run would
overflow, `log()` what was truncated; a silently dropped run reads as "this light casts no shadow," not as
a capacity limit.

## I-5 · One addressing convention, written down

Fix it once so the JS builder and the shader can't drift:
- **Def `k`** (light or caster) → band `⌊k / 1024⌋`, col `k % 1024`, occupies rows `[3·band, 3·band+2]`
  (3-px column within its band). Sample px `p` of def `k` at texel centre
  `((k%1024 + 0.5)/1024, (3·⌊k/1024⌋ + p + 0.5)/12)`.
- **LUT entry `i`** → px `i >> 2`, channel `i & 3`; px at `(px%1024, ⌊px/1024⌋)`, texel-centre uv.

## I-6 · `highp` for world positions

`world_x/_y` are world-px (thousands); `mediump` (~10-bit mantissa) can't hold them without stepping. The
caster/light samplers and the fragment math that consumes positions must be `highp` (same as the merge/
display shaders). Colour, radius, depth, frame coords are small and precision-insensitive.
