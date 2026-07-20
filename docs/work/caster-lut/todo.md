# Todo — caster-lut (execution order)

_Items move to [`completed.md`](completed.md) when done + verified. Extends `shadowCast.ts` +
`shadowCastShaders.ts`. See [`README.md`](README.md) for the layouts, [`forks.md`](forks.md),
[`issues.md`](issues.md)._

---

**C1–C4 + C6 done + verified — moved to [`completed.md`](completed.md) (2026-07-20).** The three `1024×12`
`RGBA32F` textures (light w/ `start_index`/`count`, caster data, LUT) are built + uploaded, and the light →
LUT → caster indirection drives the cast, producing shadows identical to before with no capacity warnings.

## C5 (remaining) · GPU-side cast — read the textures in a shader

- [ ] Move the wedge projection onto the GPU: a **raw GLSL ES 3.00 instanced** cast that samples the light,
      LUT and prim textures (vertex-texture-fetch) instead of the CPU mirrors. The interim CPU-mirror cast
      is landed + verified ([D-1](deviations.md#d-1)); this is the deferred half — the texture formats don't
      change, only the consumer moves CPU→GPU. First raw-shader / VTF work in the client
      ([`webgl2-es300`](../../components/client/pixijs/design/) stance: raw shaders for ES 3.00). ([F3](forks.md#f3))

## Later (optional)

- [ ] `texSubImage2D` partial uploads (prim patch in place; LUT used-prefix only) — ([I-2](issues.md#i-2)).
      Currently whole-texture `update()` on rebuild; fine at experiment scale.
- [ ] Stable caster-def free-list if casters churn (today: rebuild-all on count change) — ([I-3](issues.md#i-3)).
