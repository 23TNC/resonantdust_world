# Todo — light-data-texture (execution order)

_Planned, not started. Items move to [`completed.md`](completed.md) when done + verified. Extends
`shadow-tiered`'s `shadowCast.ts` + `shadowCastShaders.ts`. See [`README.md`](README.md) for the layout,
[`forks.md`](forks.md), [`issues.md`](issues.md)._

---

## L1 · Create the 5×5 data texture — 2026-07-20

- [ ] Add a `Float32Array(5·5·4)` + a `RGBA32F` texture over it (`nearest`, `precision highp` at sample
      time) in `ShadowCast`. Confirm the Pixi v8 API for a typed-array-backed float texture
      (`BufferImageSource`/`TextureSource`, `format: 'rgba32float'`) during build ([F1](forks.md#f1),
      [I-1](issues.md#i-1), [I-2](issues.md#i-2)).
- [ ] Writer: pack each light into its column (rows per the README table); `source.update()` to re-upload.

## L2 · Fill it every frame + randomise colours — 2026-07-20

- [ ] Each tick, write every light's position/radius/intensity into its column, and a **fresh random RGB**
      into row 3 (the per-frame change that proves the live path — [I-3](issues.md#i-3)). Re-upload.

## L3 · Display reads colour from the texture — 2026-07-20

- [ ] Pass the data texture to `ShadowTDisplayShader` as a second sampler. In the decode, for each set bit
      `k`, sample column `k` row 3 → that light's colour, and accumulate it (replacing the hardcoded
      `LIGHT_COLORS` / `DECODE` constants). Colour must come **only** from the texture.

## L4 · Verify — 2026-07-20

- [ ] `?focus=100,50&shadowcast`: shadows render as before, but each light's shadow **colour changes every
      frame** (flickering), independently per light ⇒ texture→shader data path proven.
- [ ] **Pan** → still correct (colour is per-bit; the `shadow-tiered` position/merge logic is untouched).
- [ ] Console clean — no incomplete-texture / format / `uint` errors. If the float texture fails, fall
      back to `RGBA8` ([F1](forks.md#f1), [I-4](issues.md#i-4)) and re-verify.

## L5 · (Stretch) cast/markers read position from the texture — 2026-07-20

- [ ] Optional ([F2](forks.md#f2)): source a light's position/radius from the texture rather than the JS
      `lights[]`, unifying the source of truth. Only if cheap; not a gate on L1–L4.
