# Plan — strip the lighting + shadow system

_The plan for the life of the stream. Items never move; `[x]` IS the move. Context in
[`README.md`](README.md)._

**Acceptance for the whole stream.**

- **The tree renders at every phase boundary.** A strip that leaves the client dark for three phases
  cannot tell a deletion mistake from an ordering mistake. Each phase ends at a page load.
- **Nothing is deleted before it is recorded.** P0 exists so the re-think inherits evidence rather
  than an empty folder; a measurement not taken before the code goes is gone for good.
- **No orphans.** No dead uniform, RT, band, debug hook, doc paragraph or import survives the phase
  that removed its reason to exist.

**Fixture:** area1, zoom 1, `?user=Claude&focus=100,50&zoom=1`, plus one zoomed-out load — the same
pair the shadow streams used, so before/after images are comparable.

## P0 — Record what is about to be destroyed

- [ ] Capture four reference renders: zoom 1, zoomed out, a torch-lit corner, a conifer shadow edge. Acceptance: four images in `completed.md`, each with its URL — the successor SEES what was given up.
- [ ] Record the per-frame draw sequence and the ms of each pass at N=1 and N=16 lights. Acceptance: a table — pass name, render target, ms mean and spread — from the existing tick-driven harness before any code moves.
- [ ] Record resident RT bytes per target, and the total. Acceptance: a table summing to the ~83 MiB the README claims, or a corrected figure; whichever is true is what gets written.
- [ ] Write the capability inventory: every visual behaviour the system delivers today. Acceptance: a numbered list — cold/hot tiers, mover shadows, n/s cards, shadows onto billboards, decay, per-light N·L, bilinear.
- [ ] Record the accumulated structural findings in `issues.md` with their evidence. Acceptance: each of the README's five bullets carries its number and its source, so the re-think argues with data instead of memory.

## P1 — Cut the consumers (the renderer goes unlit, nothing is deleted yet)

- [ ] Reduce the display blit to `albedo × ambient` with ambient at full ([F1](forks.md#f1)). Acceptance: the world renders at full brightness with correct colour; no lightmap sampler remains bound in the blit.
- [ ] Stop issuing the gather, lighting, receiver and decay draws from `Viewport`. Acceptance: the frame drops to its 2 remaining steady-state draws, confirmed by the draw counter, and the page still loads.
- [ ] Re-measure the frame with the lighting passes dark. Acceptance: ms before and after, which prices the whole system in one number — the budget the replacement gets to spend.
- [ ] Verify the unlit render at both fixtures. Acceptance: sprites, tiles, z-order, selection outlines and the build overlay all still draw correctly — this phase must prove the strip is reversible before anything is deleted.

## P2 — Delete the machinery

- [ ] Delete `shadowGather.ts` entirely, with its ~10 GLSL programs and ~25 debug hooks. Acceptance: the file is gone (git holds it), `tsc --noEmit` is clean, and no `globalThis.__*` lighting hook answers in the console.
- [ ] Free every lighting render target and its allocation site. Acceptance: shadow, prev-shadow, decay, receiver-coarse, receiver-fine and both lightmaps are gone; measured GPU memory drops by the P0 figure.
- [ ] Split `coldShadowData.ts`: keep the primitive-graph writer, delete the lighting-only paths ([F5](forks.md#f5)). Acceptance: the `light_data` and `light_presence` writers are gone; the graph writers and the scatter are untouched.
- [ ] Remove the lighting debug UI — panel rows, gizmos, overlays and URL parameters that no longer resolve. Acceptance: the debug panel has no dead control, and no URL parameter is silently ignored.
- [ ] Verify the deleted build. Acceptance: a page load at both fixtures, zero console errors, and the P1 render unchanged — deletion must be invisible on screen, since P1 already dimmed the passes.

## P3 — Retire the data-texture bands

- [ ] Prove `prim_presence`'s remaining consumers before touching it ([F5](forks.md#f5)). Acceptance: a list of readers; it goes only if the walks were its only ones, and stays with the reason recorded if not.
- [ ] Delete the `light_data`, `light_presence_lo` and `light_presence_hi` bands from `VARIABLES.md` and free their sets. Acceptance: sets 2, 3 and 5 read as reserved; no code writes them; the data texture's live footprint is re-measured.
- [ ] Delete the `shadow-cold` RT spec from `VARIABLES.md`. Acceptance: the section is gone rather than marked obsolete — `VARIABLES.md` holds current truth only.
- [ ] Verify the data texture end to end. Acceptance: the scatter still delivers records, the mirror matches the texture, and the world renders — the band retirement must not disturb the graph.

## P4 — Retire the documentation

- [ ] Move the four lighting/shadow design + intent docs out of the repo to `../archive/` ([F2](forks.md#f2)). Acceptance: `components/client/webgl/` describes only what exists; `rd docs-check` green.
- [ ] Update `current/` to state plainly that the client renders unlit. Acceptance: a reader who knows nothing of this stream can tell what the renderer does today without inferring it from a deletion.
- [ ] Close the eight superseded lighting/shadow work streams to `../archive/`. Acceptance: `docs/work/README.md` lists no open stream whose subject no longer exists, and each closed row says it was superseded by the strip rather than delivered.

## P5 — Leave a seam the re-think can build on

- [ ] Write `current/lighting-seam.md`: where a lighting system attaches. Acceptance: it names the draw-sequence position, the G-buffer contents, the surviving record bands, and P1's measured ms budget.
- [ ] Record the capability checklist from P0 as the acceptance any replacement inherits. Acceptance: the list sits beside the seam doc, so "does the new system do X" is answerable rather than argued.
- [ ] Record what the strip deliberately KEPT and why ([F4](forks.md#f4), [F5](forks.md#f5)). Acceptance: normal/depth maps, the G-buffer, the primitive graph and F8 each carry their one-sentence reason.
