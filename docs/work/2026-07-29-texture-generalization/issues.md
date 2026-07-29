# Issues — texture-generalization

_Defects found during execution land here. Known inputs: tile-lighting's unproven churn
suspect (an out-of-range `__torch` intensity vs the u8 [0,1] lane — clamp drills, and
consider clamping `carriedLightFor`'s store/compare for robustness); background tabs freeze
rAF and void counter drills; art-128's texture migration remains in flight._

## I1 · P1 oracle residual — ~250 words differ (RESOLVED 2026-07-29: artifact of the wedged-cache page)

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
RESOLUTION: every earlier measurement ran on a page whose texture pipeline was WEDGED (the
previewCache IndexedDB hang, fixed in `76e4bdb9` — zero stems packed, defs stuck on lod-0
solid fallbacks with art-inflated tight boxes). On a healthy, fully-settled page the oracle
reads CLEAN: cold class settle 0 / round-trip 0 / extent-vs-base 0 / brute-vs-corridor 0;
hot class (npc stopped, wolf frozen, 9,980 nz words) settle 0 / extent-vs-base 0. The
reshape is bit-identical to pre-reshape behavior in both classes. BONUS finding from the
receiver-map diff: EXTENT mode overflows the 4-slot cap on 2 tiles and silently DROPS
registrations (1,908 fine + 320 coarse receiver words where base-line correctly finds a
billboard and extent finds none) — base-line occupancy is strictly cleaner, never fuller
than 3 slots on this page. The `__fillmode` A/B toggle stays in tree (same precedent as the
DILATE-5 switch) for re-running the oracle after P2's slot re-spec.

## I2 · Post-delivery pair (user-reported, FIXED 2026-07-29)

**(a) internal_padding missed the DRAW path.** The defs/lighting applied it (offset + cell
window + `tileNormal`), but the resolver's `cellFrame` trimmed only by the MANIFEST pad
(0 by design, R5) — drawn wall cells included the 1-unit padding ring. FIX:
`resolver.setLinkedPad(name, units)` (evicts packed frames on change, the setSpriteScale
discipline); `resolve` folds it into the cell inset as whole-atlas fractions
(`units/(16·cols)`); the bridge pushes it from the DSL lanes at refreshStems for every
linked kind AND its derived blueprint sibling. VERIFIED: cell frame reads 112 px of the
128-px cell (14-unit window) — exact.

**(b) walls baked FLAT until a window move re-baked them.** A kind's def upgrade (the atlas
lod landing → a NEW slot-0 word) changed presence but dirtied NO baked lighting — billboards
have the def-swap cascade, tiles had nothing. FIX: the presence write loop compares each
tile's new slot-0 word against the mirror and coalesces changes into ONE union dirty rect
per pass (+1 ring, both classes) — a def upgrade sweeps its kind, pan-time false positives
land on already-dirty tiles. VERIFIED: a fresh reload shades the walls correctly with no
panning.
