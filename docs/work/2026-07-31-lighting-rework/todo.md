# Plan — lighting + shader rework

_The plan for the life of the stream. Items never move; `[x]` IS the move. Context in
[`README.md`](README.md); the adjustments in [`forks.md`](forks.md)._

**Acceptance for the whole stream.**

- **Every phase ends renderable, on screen, at a page load.** `tsc` cannot see GLSL and a green
  typecheck has passed broken shaders before. Nothing counts until it is looked at.
- **No MRT, anywhere.** It hung Chrome twice and was never root-caused
  ([strip I1](../2026-07-31-lighting-strip/issues.md#i1)). A phase that needs it has a design bug.
- **Every cap counts what it drops.** No silent eviction, no silent clamp ([F10](forks.md#f10)).
- **Every perf claim is measured on the same fixture**, against the stripped floor of 0.045 ms/frame,
  1 draw. A change smaller than the spread is not a result.
- **`docs/intent/2026-07-31-rework.md` owns the record layouts.** A conflict is a plan bug: log it in
  `issues.md` and raise it, do not silently diverge.

**Fixture:** `?user=Claude&focus=100,50&zoom=1` plus a zoom-0.5 load — the same pair the strip
captured its before-images at, so comparisons hold. The tab is backgrounded, so frames are driven by
hand (`__viewport.tick()`) and timing uses wall clock with `gl.finish()` at both ends, **not** timer
queries ([strip I7](../2026-07-31-lighting-strip/issues.md#i7)).

## P0 — Ground rules, formats, and the harness

- [x] Confirm the strip has reached the end of its P2 (machinery deleted, RTs freed). Acceptance: `shadowGather.ts` is gone, the page loads unlit, and the frame is 1 draw — the floor this stream builds from.
- [x] Validate `RGB10_A2` renderability + blending at boot, alongside the existing extension checks. Acceptance: an unsupported format fails LOUDLY at startup, in the same style as `renderer.ts`'s float checks — never a silent degrade.
- [x] Build the frame-cost harness: hand-driven ticks, `gl.finish()` both ends, median of 5. Acceptance: it reproduces the stripped baseline of 0.045 ms/frame within its own spread, so the instrument is trusted before it measures anything new.
- [x] Write the shared `reachFromIntensity()` used by BOTH the CPU light-set build and the GPU walk bound ([F6](forks.md#f6)). Acceptance: one implementation; a dev check asserts CPU and GPU agree at every `u10` intensity.

## P1 — The record layer: flat prims, definitions, atlas

- [x] Write the constants and the flat `u16` prim index space, index 0 reserved ([F8](forks.md#f8)). Acceptance: allocation never returns 0, and a boot assertion proves it.
- [x] Build the atlas quadtree packer with the 4 maps of a definition in one 2×2 quadrant block. Acceptance: `fullframe.span == 2 × frame.span`; a packed definition's 4 maps resolve at the offsets the design derives, verified by sampling.
- [x] Write `definition_data`: 16 sequential px per definition, indexed by rotation. Acceptance: `base + rotation` resolves to the right frame for a 4-rotation billboard AND a 16-cell linked tile, with no per-kind special case.
- [x] Write `prim_data` with a CPU-side rotation clamp against the definition's allocation ([F7](forks.md#f7)). Acceptance: a rotation past the allocation is clamped at the writer and asserts in dev — the GPU never reads a neighbouring definition.
- [x] Add dev-mode bitfield assertions to both record writers. Acceptance: a value too wide for its lane throws at write time rather than silently truncating — the failure that costs a day to find on the GPU.
- [x] Render sprites unlit from the new records. Acceptance: the zoom-1 fixture matches the strip's `after/01-zoom1-unlit.jpg` — the record layer is proven before any lighting rides on it.

## P2 — Per-tile records: presence and light

- [x] Write `presence`: 8 receivers per tile, **layer-sorted**, slot 0 = the tile. Acceptance: sorted order is asserted, because the whole per-pixel pass depends on "topmost first" and nothing else enforces it.
- [x] Write `light`: the 8 nearest light prims reaching each tile, using `reachFromIntensity`. Acceptance: a light's registered tile set matches the reach its intensity implies — the CPU/GPU agreement from P0, checked on real data.
- [x] Count and surface evictions from both caps ([F10](forks.md#f10)). Acceptance: `droppedLights` / `droppedReceivers` appear in the debug panel and go non-zero when a tile is deliberately over-subscribed.
- [x] Verify the two records against the live scene. Acceptance: a debug overlay shows per-tile occupancy; the torch registers on the tiles its reach covers and on no others.

## P3 — Lighting, no shadows yet: slots + summed map

- [x] Allocate the 8 per-light slots as `RGB10_A2` at ¼ scale ([F2](forks.md#f2)). Acceptance: a contribution of 4.0 round-trips as 1.0 stored and 4.0 read — today's overbright ceiling preserved exactly.
- [x] Draw all 8 lights in ONE draw, light index from fragment x ([F3](forks.md#f3)). Acceptance: one draw call per lighting update, no MRT, and each fragment touches exactly one slot.
- [x] Maintain the summed map by a blended delta of `new − old` ([F1](forks.md#f1)). Acceptance: the display samples ONE texture; changing one light rewrites one slot and one blended draw, not the whole map.
- [x] Prove incremental add/remove is exact. Acceptance: add a light, remove it, and the summed map returns bit-identically to its prior state — the property the old quantised accumulator needed `LIGHT_QUANT` to fake.
- [x] Measure cost per light with no shadows. Acceptance: ms at N = 1, 4, 8, 16 lights — the first real bound on the per-pixel pass, taken BEFORE anything is built on top of it.

## P4 — The shadow gather: per unit, ping-ponged

- [x] Allocate the shadow buffer at **3 px per UNIT**, ping-ponged, cleared to 0 ([F4](forks.md#f4)). Acceptance: sized ~6 MB not ~24 KB, and the first frame reads zeros rather than garbage.
- [ ] Implement the incumbent test, then adjacency, then the reach-bounded corridor walk. Acceptance: the walk runs only when both cheap paths miss; the hit rate of each is logged, so the saving is attributed rather than assumed.
- [x] Store the ground caster and the 8 `(caster, receiver)` pairs, with the corrected `l >= 4` slot split. Acceptance: all 8 lights address distinct slots; light 4 lands in px 2 slot 0, which the design's `l > 4` dropped.
- [ ] Verify the caster type lane at use ([F9](forks.md#f9)). Acceptance: forcing a stale/recycled index into a slot cannot produce a shadow — the walk re-checks rather than trusting.
- [ ] Check the gather against a brute-force reference. Acceptance: 0 differing texels between the corridor walk and an exhaustive search over the same scene.

## P5 — The per-pixel refine

- [ ] Select the receiver by **coverage first**, then the stored pair. Acceptance: a px covered only by `r6` resolves to `r6` for EVERY light — the design's early `break` picked a different surface per light.
- [ ] Hoist surface selection out of the light loop, now that it is light-independent. Acceptance: `r.has(px)` is evaluated once per pixel, not up to 8 times, in the pass that dominates cost.
- [ ] Gate the refine on the unit holding a caster ([F5](forks.md#f5)). Acceptance: the gate's selectivity is reported as a measured percentage of pixels, and interior/fully-lit pixels do no texture test.
- [ ] Measure the refine's own cost. Acceptance: ms attributable to the refine alone at N = 1 and N = 8, against P3's no-shadow baseline.
- [ ] Compare the shadow edge against the strip's before-image. Acceptance: `before/04-shadow-edge-zoom2.jpg` beside the new render — the edge should be silhouette-exact where the old one stepped at 16/tile.

## P6 — Errors, limits, and what a failure looks like

- [ ] Clamp every contribution to the storable range before write, guarding NaN/Inf. Acceptance: a deliberately NaN-producing light writes a bounded value and does not poison the summed map through the blend.
- [ ] Audit every record fetch for the index-0 guard ([F8](forks.md#f8)). Acceptance: a grep-backed list of fetch sites, each with its guard — an unguarded fetch of prim 0 is the cheapest silent corruption available.
- [ ] Surface the drop counters, the gate selectivity and the walk hit rates in the debug panel. Acceptance: one panel answers "is this scene over its caps, and where is the time going" without a code change.
- [ ] Write the failure-mode table into `issues.md`: for each cap and guard, what the user SEES when it trips. Acceptance: a reader can go from a visual symptom to the limit that caused it.

## P7 — The verdict

- [ ] Re-measure moving lights at reach 16, zoom 1, inside 8 ms. Acceptance: the largest N that fits, plus the ms at N and N+1 — against the old system's 15–16.
- [ ] Measure resident bytes across every new map. Acceptance: a table against the 83 MiB the strip freed, so the rework's real footprint is known rather than estimated.
- [ ] Record the capability A/B against the strip's four before-images. Acceptance: same camera, same content, side by side — and an honest sentence on anything the new system does NOT do.
- [ ] Update `VARIABLES.md` with the shipped record layouts. Acceptance: it describes what the code writes, and `docs/intent/2026-07-31-rework.md` retires to `../archive/` once its content lives in the authoritative file.
