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
