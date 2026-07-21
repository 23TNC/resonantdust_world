# Deviations — caster-lut

_Logged at the moment of deviating, per the docs convention. Each: what the plan said, what was done, why._

---

## D-1 · The cast reads the CPU mirror this milestone, not the GPU textures — 2026-07-20

**Plan ([C5](todo.md), [F3](forks.md#f3)):** a shader cast that reads light → LUT → caster **from the
textures on the GPU** and projects the wedge.

**Done:** C1–C4 in full — the three `1024×12` `RGBA32F` textures are built, maintained and **uploaded**
(real, GPU-ready) — and the cast is driven by the **light → LUT → caster indirection**, but reading the
**CPU mirrors** (`lightData`/`lutData`/`primData`, byte-identical to the uploaded textures) rather than
sampling the textures in a shader. The projection stays in the JS `Graphics` cast.

**Why:** the GPU read of these data textures needs **vertex-texture-fetch in an instanced draw**, which on
this stack means a **hand-written raw GLSL ES 3.00 shader** (Pixi's high-shader is ES 1.00, and VTF there is
`texture2DLod`-only / not exposed by the `texture` macro — the `webgl2-es300` finding). That's a
substantial new capability (every shader in the client is high-shader ES 1.00 today) and high-risk to
one-shot. Splitting it keeps this milestone **verified** (the data structures + the full indirection are
proven to produce correct shadows) and isolates the raw-shader work as its own focused step. This is
exactly the F3 "verify the build first, then the GPU read" fallback.

**Follow-up:** C5 (the raw-ES3 instanced GPU cast) is re-scoped as the next step and remains open in
[`todo.md`](todo.md). The texture formats/layouts don't change — only the consumer moves from CPU to GPU.

## D-2 · C5a spike re-scopes C5 — two Pixi-pipeline blockers found — 2026-07-20

**Plan:** C5a proves `texelFetch` + integer RT + vertex-texture-fetch, then C5b–c build the integer bitfield
+ instanced GPU cast on them.

**Found (`es300IntSpike.ts`, `/inttest`):** the spike isolated each technique.
- **Works** through a raw ES 3.00 `GlProgram` mesh: instancing + per-instance attributes, float render
  targets, and `texelFetch` in the FRAGMENT + a full-screen display. (5 distinct colour-quads render.)
- **VTF fails via a raw program** ([I-10](issues.md#i-10)): `texelFetch(uData)` in the VERTEX stage returned
  0 — Pixi didn't bind the texture resource to the raw program's vertex sampler (the quad collapsed to clip
  (0,0)). The instanced wedge cast reads the caster/light records in the vertex, so it needs this.
- **Integer render targets fail via Pixi's mesh render** ([I-11](issues.md#i-11)): rendering `uvec4` into an
  `RGBA8UI` target → `GL_INVALID_OPERATION`.

**Re-scope:** C5 is bigger than "a shader that reads textures." The GPU cast needs **either** the high-shader
ES 3.00 path (which binds textures correctly — a VTF vertex bit on it) **or** raw GL; the integer bitfield
(facet 2) needs **raw-GL rendering** (own FBO + `clearBufferuiv` + draw state), bypassing Pixi's mesh pipe.
Options for the next step: (a) resolve VTF via the high-shader path + do the integer target in raw GL;
(b) do the whole cast in raw GL; (c) **de-scope facet 2** — build the GPU cast first writing the existing
**unorm** bitfield with additive-blend OR (which works today), and revisit integer storage separately (the
float-mod retirement then stays a `uint`-on-unorm cleanup, not an integer-texture switch). Pending a call.
