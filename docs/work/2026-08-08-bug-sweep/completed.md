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
