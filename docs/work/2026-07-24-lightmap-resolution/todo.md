# Todo — full-resolution lightmap (execution order)

_Model + framing in [`README.md`](README.md); decisions in [`forks.md`](forks.md). A/B fine vs coarse at
every visible step (`__finelight`). The shadow gather + `shadow-cold` are NOT touched — only lighting
accumulation + storage. VERIFY perf at each step: the bake grows (fine res × per-light `N·L`), so watch
that dirty-gating still means a static scene re-bakes nothing._

## P0 · Sample the prim normal from its atlas frame in the bake ([forks.md#f3](forks.md#f3), [#f3a](forks.md#f3a)) — ✅ DONE 2026-07-24
- [x] Mechanism resolved: **parallel `NORMAL_DEF_BASE` band** (per-def normal frame origin, refreshed on the
      per-tick caster walk) — the immutable def can't carry it ([#f3a](forks.md#f3a)). `bin/art` is aligned to 55°
      but the **corpus normals are still RAW** (unpitched) — P1's `N·L` verifies against a **temporary in-shader
      pitch** (dev), corpus re-bake to world-frame is the P3 follow-up ([forks.md#f7](forks.md#f7)).
- [x] Bound `uSurface` + `uNormal` to `LIGHT_FRAG`; `primNormal()` reuses the silhouette's frameRel (only the
      frame ORIGIN swapped) + `texelFetch`es the normal. W-facing mirrors `n.x`.
- [x] VERIFIED live: the sampled normal reads right per prim (each tree/bush shows its own coherent normal map),
      **rock-stable across a full zoom sweep both directions** — frame-indexed, NOT a world-coord composite read.
      No console errors; non-debug path undisturbed. Debug hook `__shownormal`.
- [ ] (Optional, [forks.md#f6](forks.md#f6)) co-pack albedo/normal/surface into one atlas — the F3-A band's
      eventual consolidation, sequenced after the lighting correctness lands.

## P1 · Bake per-light `N·L` into a fine, single-attachment lightmap
- [ ] Resize the cold/hot lightmap RTs to `cols·R × rows·R` (`R` = `TEXTILE_SQUARE`, [forks.md#f4](forks.md#f4)),
      **1 irradiance attachment** each (drop att1 direction + att2 unshadowed — subsumed by baking `N·L`;
      [forks.md#f5](forks.md#f5)). HDR format if accumulation overflows (many lights).
- [ ] `LIGHT_FRAG` at fine res: per presence light, `color·falloff·(1−shadow)·max(0, N·L)` with the fine
      normal (P0) and the world-space `dir_to_light` ([world-space-lighting](../2026-07-24-world-space-lighting/README.md)),
      **upsampling** the coarse `shadow-cold` per light ([forks.md#f4](forks.md#f4)). Sum → the texel.
- [ ] Dirty-gating reworked for the new slot `R` (cold/hot independent; a normal change also dirties).

## P2 · Collapse the blit to `albedo × lightmap`
- [ ] Blit reads the fine lightmap by world position (its own `R`), multiplies albedo, adds ambient. Remove
      the blit-side relief / direction / unshadowed path (now baked in). Keep `__finelight` to A/B the whole
      new pipeline against the old aggregate one.
- [ ] VERIFY: per-light-correct lighting (a normal facing light A but away from B lights from A only, not
      the average); crisp; no zoom drift.

## P3 · Scale + verify the spend
- [ ] Push the light count UP (the whole point) — confirm many static lights cost ~nothing per frame
      (dirty-gated bake), and a moving light only re-bakes its reach. Measure fps at the display cap + the
      lightmap VRAM (write the MB numbers post the 4×→2× zoom reduction; [forks.md#f5](forks.md#f5)).
- [ ] The A/B is the deliverable: fine per-light vs coarse aggregate, side by side on a many-light forest —
      confirm the correctness + detail is visibly worth the VRAM ([issues.md#i2](issues.md#i2)).
- [ ] Docs + memory. Note the shadow stayed coarse + the albedo untouched (both intentional).
</content>
