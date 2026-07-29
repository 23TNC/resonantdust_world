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
