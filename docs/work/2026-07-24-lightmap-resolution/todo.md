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
_Split into Step A (correctness, current res) + Step B (resolution) so each step has ONE variable._
### Step A · per-light `N·L` at the CURRENT res + blit collapse — ✅ DONE 2026-07-24
- [x] `LIGHT_FRAG` bakes `Σ colour·falloff·(1−shadow)·max(0, N·L)` per light on THINGS (ground keeps falloff,
      `ndl = 1` — unchanged; ground-as-prims `N·L` is a later step). World-space `dir_to_light`
      (`toL.x, toL.y·nsInv, Lz`) dotted against the world-frame normal. `worldNormal()` maps sprite-tangent →
      world (rotate the flat basis up by `90°−tilt` about east); corpus is RAW so the pitch is DEV in-shader
      (`__pitchnormal`, [forks.md#f7](forks.md#f7)) until the re-bake.
- [x] Blit collapse gated on `uNLbaked`: `light = ambient + irr` (the aggregate-direction relief path is
      bypassed — it was the lossy stand-in). Old path retained under `__finelight(false)` for A/B.
- [x] VERIFIED live: things show per-light directional facet shading (was flat aggregate glow); pitch sign
      correct (highlights on light-facing facets); zoom-stable; A/B toggle works. Dimmer (inherent to Lambert).
### Step B · bump to fine resolution (`TEXTILE_SQUARE` = 64/tile) — ✅ DONE 2026-07-24
- [x] Light RTs resized to `cols·TEXTILE_SQUARE × rows·TEXTILE_SQUARE` (`FINE_RATIO` = 4× the coarse shadow RT,
      decoupled). `LIGHT_FRAG` maps its fine `fc`→world at `uSlot·FINE`; **upsamples** the coarse `shadow-cold`
      at `fc/FINE` per light. Blit reads the fine lightmap at `uLSlot = TEXTILE_SQUARE`.
- [x] Dropped att1 (direction) + att2 (unshadowed) — subsumed by baking `N·L`; **1 `rgba8unorm` attachment**
      each ([forks.md#f5](forks.md#f5)). LDR clamp holds so far; HDR deferred to P3 if many-light bands.
- [x] Dirty-gating unchanged in shape (still per-tile via `uDirty`); the fine slot only rescales `fc→tile`.
- [x] VERIFIED live: crisp per-facet tree lighting (the bilinear smear is GONE — nearest at fine res),
      zoom-stable both directions, no errors. Ground shadows stay coarse-edged (shadow map is still 16/tile).

## P2 · Collapse the blit to `albedo × lightmap` — ✅ DONE 2026-07-24 (merged with Step B)
- [x] Blit is now `out = albedo × (ambient + coldLight + hotLight) × alpha`, sampled **NEAREST** at fine res.
- [x] DELETED the whole aggregate path: `irrBilinear`, relief (`uColdDir`/`uHotDir`/`uNormal`/`uReliefStrength`),
      the `#3` depth-test restore (`uColdUnshadowed`/`uHotUnshadowed`/`uZDepth`/`inFront`/`uDepthTest`),
      `uPrimShadow`, and the `flat` fallback texture. Their `ShadowGather` getters + `Viewport` hooks
      (`__relief`/`__depthtest`/`__primshadow`/`__finelight`) are gone too (delete-don't-deprecate; git holds it).

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
