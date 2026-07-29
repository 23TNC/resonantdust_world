# Issues — texture-generalization

_Defects found during execution land here. Known inputs: tile-lighting's unproven churn
suspect (an out-of-range `__torch` intensity vs the u8 [0,1] lane — clamp drills, and
consider clamping `carriedLightFor`'s store/compare for robustness); background tabs freeze
rAF and void counter drills; art-128's texture migration remains in flight._

## I1 · P1 oracle residual — ~250 words differ (OPEN; white-box instrumentation needed)

The reload oracle is INVALID (lod retention makes def settling arrival-order-dependent —
same build varies 1,639 words across reloads); the VALID oracle is the in-page `__fillmode`
toggle: extent registration + uDilateS 0 (the EXACT pre-reshape behavior) vs base-line +
dilation, same settled defs. CONTROLS both clean (base↔base 0 diffs, extent↔extent 0 diffs
across toggle round-trips — deterministic and wolf-immune). RESIDUAL: 249–270 words
(~0.9 % of nz), old-finds-more in ~90 % of them, ALL in slot 1 (the (104,59) torch)
clustered at tiles (110–111, 57), PARTIAL penumbra values (13 vs 58, 25 vs 127…).
ELIMINATED: slot capacity (max occupancy 2), >uDilateS spans (live maxTightHpx 256 → span
≤ 2; a dilation-5 run changes NOTHING — 249 diffs persist), cold ns casters (none),
registration sets (CPU dump matches the model), found-caster sets (a JS DDA mirror of the
fetch sets reports ZERO discrepancies for the affected ray), bref sensitivity
(`resolvedTilePos` is nearest-congruent, refs within 3 tiles), corridor-vs-brute (0 diffs
per mode). CONCLUSION: the same casters evaluate to different coverage between modes
through an input I haven't identified — next step is white-box: instrument `casterOne`/
`casterCover` for ONE texel+light (e.g. RT word 302024, light slot 1) to dump per-caster
(cc, frac, uMid, A, bref) under each mode and diff. KEPT IN TREE: the `__fillmode` oracle
toggle (extent + uDilateS-0 = old-exact) and the live `maxTightHpx` dilation bound (a real
robustness fix regardless — art-inflated tight boxes exceeded the content-nominal card).
NOTE: the world's ART is garbage during all of this (blockers B1) — the oracle is valid on
garbage (deterministic silhouettes), but any VISUAL judgment stays blocked.
