# lighting-feel — todo

_Plan for the stream (see [README.md](README.md)). A/B = screenshot pair at the `area1` torch
cluster (`?user=Claude&focus=100,50&zoom=1&cb=area1`), before/after, recorded in completed.md.
GPU timing = the EXT_disjoint_timer_query harness from
[lighting-standing-costs](../2026-07-27-lighting-standing-costs/completed.md). Opened 2026-07-27._

## P0 — audits (settle the art-gated questions first)

- [x] Audit corpus AO: decode surface-G over a sample of co-packed frames (browser readback or bin/art) and report the per-kind value distribution. Acceptance: a table in issues.md saying whether G carries structure or is ~flat.
- [x] Audit emissive: count stems with emissive leaves on disk/manifest (expected ~0). Acceptance: counts in issues.md; P3's art items are sized from this.

## P1 — accumulateLights knobs

- [x] Colour temperature: per light, warm the colour at the core and dim+desaturate toward reach as a function of the existing falloff value; live-tunable via a debug hook. Acceptance: A/B at area1; GPU timing within noise of baseline.
- [x] Specular glints: Blinn N·H per light on things (reuse the live `billboardNormal`), small intensity, off on ground. Acceptance: visible glint on foliage near a torch in A/B; timing within noise.

## P2 — the decay lightmap

- [x] Add the coarse decay RT ([F1](forks.md#f1)) + the in-place decay draw ([F2](forks.md#f2), dt-derived k). Acceptance: a debug splat fades smoothly to zero; decay pass ≤ 0.05 ms (GPU-timed).
- [x] Blit adds the decay map, bilinearly upsampled, AFTER the accumulator sum. Acceptance: pixel-identical output when the map is empty; soft glow when not.
- [x] Splat path: additive particle quads (world pos, radius, colour, intensity), shadow-STAMPED by the parent light's coarse-shadow slot at emit ([F4](forks.md#f4)). Acceptance: a splat emitted over a tree shadow shows the shadow cut into its glow.
- [x] Flicker emitter v1 ([F5](forks.md#f5)): lights with a flicker flag emit jittered splats; the base light stays STATIC in the hot accumulator. Acceptance: torches at area1 visibly flicker with `debugClassDraws 0` on static frames; frame rate unchanged.
- [x] Toroidal correctness: splats + decay survive pan/zoom (window remap) without smears or wrap ghosts. Acceptance: pan across a flickering torch + zoom sweep 1→0.25→1, no artifacts, no GL errors.

## P3 — AO + emissive (sized by P0)

- [x] If the AO audit shows structure: multiply the blit's AMBIENT term by surface-G. Acceptance: A/B grounding under trees; if G is flat, record that and reroute to the next item.
- [x] bin/art: strengthen/bake AO into surface-G (Laigter param or a dedicated bake step), re-publish the corpus sample. Acceptance: the P0 audit re-run shows structure; A/B shows grounding.
- [x] bin/art: emit emissive leaves for lit kinds (torch flame first); resolver + co-pack carry them ([F3](forks.md#f3) resolves here); blit adds emissive AFTER the light multiply. Acceptance: the flame glows inside a shadowed area.

## P4 — intent capture + wrap

- [x] Review [`docs/intent/lighting-feel/`](../../intent/lighting-feel/README.md) (written at planning) against what P1–P3 actually shipped; amend where execution taught better. Acceptance: `bin/rd docs-check` green; the doc reflects the built reality.
- [x] Record final A/Bs + timings in completed.md; update the work index row. Acceptance: docs-check green; every P1/P2 change has its screenshot pair.
