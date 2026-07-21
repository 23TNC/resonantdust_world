# Todo — caster-lut (execution order)

_Items move to [`completed.md`](completed.md) when done + verified. Extends `shadowCast.ts` +
`shadowCastShaders.ts`. See [`README.md`](README.md) for the layouts, [`forks.md`](forks.md),
[`issues.md`](issues.md)._

---

**C1–C4 + C6 done + verified — moved to [`completed.md`](completed.md) (2026-07-20).** The three `1024×12`
`RGBA32F` textures (light w/ `start_index`/`count`, caster data, LUT) are built + uploaded, and the light →
LUT → caster indirection drives the cast, producing shadows identical to before with no capacity warnings.

## C5 · GPU cast + integer-texture bitfield (three coupled facets — see the README)

### C5a · Spike the new ES 3.00 techniques — 2026-07-20

- [ ] A small `/command` spike (like the MRT B1) proving all three genuinely-new techniques in isolation
      before the real cast: **`texelFetch`** (exact integer-coord read of an `RGBA32F` data texture — no
      filtering); an **integer render target** (`RGBA8UI`/`RGBA32UI` — sample a `usampler2D`, write `uint`
      via real bitwise); and **vertex-texture-fetch** (sample a texture in the VERTEX stage of an instanced
      draw — [I-9](issues.md#i-9)). If any is shaky, resolve it here, not in the cast.

### C5b · Integer-texture bitfield — retire float-mod — 2026-07-20

- [ ] Convert the shadow bitfield RTs (screen-shadow, world-shadow) from unorm RGBA8 to integer
      (`RGBA8UI`/`RGBA32UI` — [F4](forks.md#f4)). Rewrite EVERY shader that reads the bitfield —
      `ShadowMergeShader`, `ShadowTDisplayShader`, the overlay BITS mode, the decode filter — to
      `usampler2D` + `uint` bitwise, **retiring float-mod** ([D-2 lands here](../es300-migration/deviations.md);
      [I-8](issues.md#i-8)). Verify shadows render identically at the current light count.

### C5c · GPU instanced cast — 2026-07-20

- [ ] Replace `castScreen` (JS `Graphics`) with the ES 3.00 instanced shader: per (light, caster) read the
      light + LUT run + caster record (`texelFetch`/VTF), project the wedge, write the light's bit. OR
      without fixed-function blend ([I-7](issues.md#i-7), [F5](forks.md#f5)) — the merge already shader-ORs;
      the screen cast writes a single dirty light's bit. Verify shadows identical to the CPU-mirror cast;
      move-a-light + pan still correct (the shadow-tiered invariants hold).

### C5d · Verify + prove the ceiling lifted — 2026-07-20

- [ ] `grep 'mod(floor('` in the shadow shaders → **empty** (float-mod gone). Bump the experiment past 24
      lights (e.g. seed 32) to prove the integer bitfield's higher ceiling. Full sweep: display,
      `/overlayRT shadow-a`, `/shadowcast`, pan — all correct.

## Later (optional)

- [ ] `texSubImage2D` partial uploads (prim patch in place; LUT used-prefix only) — ([I-2](issues.md#i-2)).
      Currently whole-texture `update()` on rebuild; fine at experiment scale.
- [ ] Stable caster-def free-list if casters churn (today: rebuild-all on count change) — ([I-3](issues.md#i-3)).

## Later (optional)

- [ ] `texSubImage2D` partial uploads (prim patch in place; LUT used-prefix only) — ([I-2](issues.md#i-2)).
      Currently whole-texture `update()` on rebuild; fine at experiment scale.
- [ ] Stable caster-def free-list if casters churn (today: rebuild-all on count change) — ([I-3](issues.md#i-3)).
