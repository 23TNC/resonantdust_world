# Completed — bug sweep

## 2026-08-08 — P0/P1: the z-order paper + panels

- VARIABLES.md carries the tier table (32 game / 40 details+inventory / 48
  chat+build / 56 settings+debug / 64 chrome) and the ordering law. Verified:
  docs-check green.
- DomPanel: named bands + `_onTop` + the ontop escalation REPLACED by numeric
  `zOrder` tiers — z = tier × 10k + tier-recency; `bringToFront` advances only
  its tier's counter; layerUp/Down clamp inside the tier; the ensureVisible
  escalation is a plain in-tier raise (cross-tier burial is impossible by
  construction). On Top deleted whole: the popup row, the listeners, the locale
  string; `<key>.onTop` storage is never read (I1 — verified in a browser that
  HAD ontop-band panels: details loads at 400001 in its tier).
- All ten constructor sites tiered (I8): viewport 32, details/inventory 40,
  chat/build 48, settings/debug/video 56, settings-popup + login form CHROME.
  Verified live: Game 320001 < Details 400001 < Build/Chat, and the 48-tie
  flipped by last click both ways (480002→480003→480004). FOUND en route: the
  taskbar's hardcoded z 50001 sank below the new tiers — moved to chrome
  (640001), verified above everything.

## 2026-08-08 — P2: the geo flash

- ROOT CAUSE (read, not guessed): `setSubframe` EVICTED the stem's pack on
  every facing re-registration — the mover re-registers cell 0's rect per
  turn, and `resolve` answered GEO for each repack window (bytes cached
  throughout: "not a texture load", exactly as reported). Fix: the OLD pack
  keeps serving (only the size guard reopens); a generation stamp discards an
  in-flight pack cut with an outdated rect (serves once, re-kicks). Probe: geo
  answered for an EVER-PACKED stem counts on `__geoSwapFlashes` + a console
  warn. Verified: a minute of live npc wander (dozens of facing flips) →
  ZERO; first-load geo untouched. Recorded I9 (the deeper pre-existing
  cell-0 facing fight → repack churn; per-facing resolve cells = successor).
- The user's mid-stream report (pie menu invisible): fixed-position chrome
  had hand-rolled z literals (menu 60000, tooltips 60001) that sank under the
  game view's tier z — all chrome now rides `Z_CHROME_BASE` (I10). Verified:
  the pie menu renders over the world again (captured).

## 2026-08-08 — P3: solid walls

- `pathable = false` on wall_smooth (one flag; the derived law carries it).
  Golden: exactly `tile wall_smooth` joins the impathable list. Drilled: the
  pen interior (105,62) refused (F5 log); a (98,67)→(110,62) cross-pen trip
  arrived AROUND the wall with a 120 ms auth+render monitor counting ZERO
  wall-tile hits.

## 2026-08-08 — P4: outlines occlude

- One outline item per OBJECT: ≤4 parts' surface silhouettes form a UNION
  (per-part world rects/frames/flips; ES 3.00 constant-indexed samplers); a
  fragment lights only OUTSIDE all parts and ADJACENT to at least one.
  Captured: the selected human's ring hugs head+body with a clean neck (the
  user's screenshot resolved); the wolf's single-prim ring unchanged; tiles
  keep the box ring; surface-less placeholders fall back to a carrier box.

## 2026-08-08 — P5: the vanishing wolf

- The evidence file is I11: a 100 ms all-mover probe (missing prims /
  zero-area / lost texName / mover GONE) through 4 min of cross-seam trips +
  2 min of 9-spot camera churn — ZERO events, eliminating prim loss,
  zero-size resolves, texName loss, and zone-close drops. The STRONG
  candidate is P2's root cause (continuous pre-fix repack churn — the I9
  cell-0 fight — yielding intermittent geo/misdraw during movement),
  plausibly cured by the P2 fix. The re-armable probe snippet lives in the
  issue for the moment anyone SEES a vanish again.
