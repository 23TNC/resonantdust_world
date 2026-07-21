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

## I-7 · Integer render targets have NO fixed-function blend — OR in-shader

`RGBA8UI`/`RGBA32UI` targets can't use the fixed-function blender, so the **additive-blend OR** that
`castScreen` uses to combine lights' bits (disjoint bits → `add` == OR) is gone. The OR must be done in a
shader. The **merge already does** (reads prev-world + prev-screen, ORs, writes cur-world — no blend), so
the world bitfield is fine; the **screen cast must become a shader** (C5c facet 3) and write bits directly.
This is why the integer switch (C5b) and the GPU cast (C5c) are inseparable.

## I-8 · The integer switch ripples across EVERY bitfield-reading shader

Switching the bitfield storage to integer is not local to the cast. **All four** shaders that currently
read the RED byte with float-mod must convert to `usampler2D` + `uint` bitwise: `ShadowMergeShader`,
`ShadowTDisplayShader`, the `overlayShader` BITS mode, and the `makeShadowDecodeFilter`. Plan C5b as "flip
the storage + rewrite these four together", verified identical, before the cast (C5c) writes into it. This
is where the `es300-migration` D-2 float-mod-→-uint cleanup actually lands (uint is load-bearing now).

## I-9 · Vertex-texture-fetch — sample the caster/light textures in the VERTEX stage

The instanced cast builds each caster's wedge quad in the VERTEX shader, so it samples the light + caster
records there (VTF). WebGL2 guarantees `MAX_VERTEX_TEXTURE_IMAGE_UNITS ≥ 16`, so VTF is available — but
confirm in the C5a spike. Use `texelFetch` (integer coords, no LOD) for vertex sampling (no derivatives in
the vertex stage, so `texture()` with implicit LOD is invalid there; `texelFetch`/`textureLod` are the
vertex-safe reads). The data textures are `RGBA32F` (`highp`) — exact for the indices + world-px positions.

## I-10 · VTF via a raw `GlProgram` didn't bind the texture (C5a)

`texelFetch(uData, …)` in the VERTEX stage of a raw `GlProgram` mesh returned 0 — Pixi bound the texture
resource for the fragment path but not the vertex sampler (or not at all for a raw program). The
instanced wedge cast reads caster/light records in the vertex, so VTF is required. Resolve by using the
**high-shader ES 3.00 path** (`compileHighShaderGlProgramES300` — its texture binding is proven by the
material/shadow shaders) with a VTF **vertex bit**, or by driving the draw in **raw GL** (bind the sampler
uniform + texture unit ourselves). Not a WebGL2 limitation — VTF is standard; it's a Pixi-binding gap.

## I-11 · Integer render targets error through Pixi's mesh render (C5a)

Rendering a `uvec4` fragment output into an `RGBA8UI` `RenderTarget` via `renderer.render({ target })` →
`GL_INVALID_OPERATION`. Pixi's mesh pipeline (blend/clear/state) isn't set up for integer attachments (an
integer FBO needs `gl.clearBufferuiv`, blend forced off, matching output types). The integer bitfield
(facet 2) therefore needs a **raw-GL** cast/combine — own framebuffer, own clear + draw state — not Pixi's
mesh render. Weigh this cost against de-scoping facet 2 ([D-2](deviations.md#d-2) option c).
