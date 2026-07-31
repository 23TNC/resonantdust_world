# lighting-standing-costs — todo

_Plan for the stream (see [README.md](README.md)). Fixture throughout: the calibrated orbit harness
(one moving light, 6-tile orbit, zoom 1, 240 frames, cold draw discriminated by render target) from
`plane-intersection`; identity = corridor↔brute
`debugReadShadow` at 0 mismatches. Opened 2026-07-27._

## P0 — baseline

- [x] Measure the static-frame standing tax with the GPU timer, orbit OFF: total ms of the 6 unconditional submissions (2 prev-blits + 2 gathers + 2 fine draws) on a frame with zero dirty tiles. Acceptance: the figure recorded in completed.md.

## P1 — delete the dead cost

- [x] Track per-class dirty counts in `buildDirty` and skip a class's ENTIRE `classPass` (blit + gather + lighting draw) when its count is 0. Acceptance: static scene submits no lighting draws (instrumented); orbiting scene unchanged; identity 0.
- [x] Gate the prev-shadow blit OFF until the differential is wired (`uShadowPrev` has no consumer today). Acceptance: no blit in a frame trace; identity 0; the flag's comment names the differential as the re-enabler.
- [x] Delete the vestigial `oCasterD` attachment (nothing reads `textures[1]`) from `GATHER_FRAG` and all four shadow RTs. Acceptance: `bin/rd check` green; `/overlayRT shadow-cold` unchanged; identity 0.
- [x] Re-run the reach sweep (4/8/12) and record the delta against the `light-budget I2` table. Acceptance: table in completed.md.

## P2 — draw dirty rects, not the window

- [x] Merge each class's dirty mirror into tile-aligned rects CPU-side (greedy row-merge suffices). Acceptance: unit-of-work log shows rect count ≤ dirty tiles and rect area = dirty area exactly.
- [x] Draw the merged rects as instanced quads in ONE draw for BOTH the gather and the fine pass, replacing the fullscreen quad ([F1](forks.md#f1)). Acceptance: identity 0; output bit-identical to the fullscreen build.
- [x] Retire the `uDirty` texel gate from both shaders once the rect path has held identity (belt-and-braces during transition only). Acceptance: no `uDirty` fetch remains in `GATHER_FRAG`/`LIGHT_FRAG`; identity 0.
- [x] Instrument fine fragments rasterized per frame and confirm ≈ dirty texels (not window texels); re-measure the reach-4 row. Acceptance: fragment count + ms recorded; the discard-tax share is now quantified.

## P3 — the persistent receiver map

- [x] Add a receiver-dirty mirror (R8UI, per tile) fed ONLY by `markPrimDirty`/`markBillboardDirty` — never by light moves ([F2](forks.md#f2)). Acceptance: orbiting a light leaves it empty; moving/adding/removing a billboard marks its rect.
- [x] Bake the receiver map: a FINE RT of (billboard id, coverage, baseY, world normal), written by a pass running `receiverAt` + `billboardNormal` under the receiver-dirty gate. Acceptance: `/overlayRT` decode shows ids + normals; zoom-sweep stable.
- [x] Convert `LIGHT_FRAG` to read the receiver map — one fetch replaces the 6-row bucket scan + record decodes + normal derivation. Acceptance: lightmap bit-identical to the live-scan build on a static scene; identity 0.
- [x] Convert `GATHER_FRAG` likewise, including the `allBillboard` corner test (bake the flag or read the 3 neighbour texels). Acceptance: shadow RT bit-identical to the live-scan build; identity 0.
- [x] Re-measure ms per tile-light pair on the reach sweep and record the new constant. Acceptance: table in completed.md, beside the old 0.0015.

## P4 — wrap + re-sync the budget

- [x] Re-run the `light-budget P0` pair-cost sweep on top of P2+P3, sweeping REACH too (`I3`). Acceptance: light-budget's I2 table updated or superseded there.
- [x] Record final numbers + any retired options in completed.md. Acceptance: `bin/rd docs-check` green.
