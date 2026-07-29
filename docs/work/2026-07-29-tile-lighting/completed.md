# Completed — tile-lighting

_Dated entries, appended as items land: what landed and how it was verified._

## 2026-07-29 · P0 — the audit (2/2)

THE MECHANISM, pinned: `SquareCache.standingPrims()` (`SquareCache.ts:551`) returns only
prims with `zIndex >= 1` — "standing" IS the zIndex threshold. Things carry `thingZ ≥ 1`;
tiles are ground at zIndex 0, so no tile ever reaches `buildCasters` — records never mint
(the live probe: the wall prim renders, resolves def, `inStanding: false` among 662 standing
prims). A naive bypass would hand a floor tile the LEANING tree card — hence F1 (standard
card, height-gated) and F2 (the `&tile.height` lane via `Primitive.litTile`; `white` ground
authors nothing and stays bit-identical — the D2 gate). F3 resolves the blueprint half:
display-time lightmap sampling in the overlay, no records (ephemerality by construction).

## 2026-07-29 · PAUSED at P1 (user) — superseded by the texture-generalization redesign

What LANDED and stays: the P0 audit (the zIndex≥1 standing gate, file:line), the
`&tile.height` lane end-to-end (content + loader + wasm + bridge table — dormant, reusable),
AND the manifest-race fix (tile-lighting I1, a REAL shipped-feature bug: walls painted
bare-stem/no-cell when overrides raced the manifest — `linkedUnknown` + `onTexturesChanged`
re-expand rows and repaint ground overrides once the grid lands; verified live: fresh reload
now yields `/l` textures, per-cell cells, lod-7 defs). What was LEARNED and fed the
redesign: per-(cell,lod) defs hit the two-lane conflict (offset lanes wanted for BOTH
placement and sampling) and the normal-QUADRANT stride mismatch (2^lod = the cell side vs
the co-pack stride = the whole atlas side); a mid-drill churn scare traced to probes running
on a background tab (rAF frozen — measurements void) plus an out-of-range __torch intensity
(u8 [0,1] lane) as the suspected re-dirty loop — unproven, noted for the successor. The
litTile participation gate is PARKED (standingPrims reverted; tilePrimSpec no longer sets
it): the successor puts tiles into lighting via prim_presence slot 0 + definition_data.
