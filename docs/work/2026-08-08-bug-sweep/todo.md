# Plan — bug sweep

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions
in [`forks.md`](forks.md) (F#), the anticipated-issue inventory in
[`issues.md`](issues.md) (I#)._

## P0 — the paper

- [x] VARIABLES.md: the panel Z-ORDER table (F1 — tiers 32/40/48/56, tier-recency
      tie-break, chrome above all, On Top REMOVED) beside the details-panel layout
      notes. Acceptance: docs-check green. → the table + the ordering law; green.

## P1 — panel z-order

- [x] DomPanel: numeric `zOrder` replaces the named bands — z = zOrder × stride +
      tier recency; `bringToFront` advances its TIER's counter only (F1).
      Acceptance: unit-ish probe — two tiers, four panels: clicks reorder within a
      tier, never across. → live probe: Game 320001 < Details 400001 < Build/Chat
      48xxxx; layerUp/Down clamp to the tier's stride.
- [x] DELETE On Top: the settings-popup row, `_onTop`, the `ontop` band, the
      storage key (ignored on load — I1). Acceptance: a browser with a stored
      onTop=true loads with the panel in its tier; no On Top row in settings. →
      this browser HAD ontop-band panels (details at 50001 pre-change); it loads
      at 400001 in its tier — the key is never read; the row, listeners, locale
      string all deleted.
- [x] Assign the user's tiers at every constructor (I8): game view 32, details 40,
      inventory 40, chat 48, build 48, settings 56, debug 56. Acceptance: captures —
      details over game, chat over details, settings over chat; chat vs build ties
      flip by last click. → all ten sites (incl. video-settings 56, popup + login
      form as CHROME 64); the tie-flip probed live: Build click → 480003 over
      Chat 480002; Chat click → 480004 back on top. FOUND: the taskbar's
      hardcoded 50001 sank below the new tiers — moved to the chrome band.

## P2 — the geo flash

- [x] Instrument the warm re-bake: a probe distinguishing first-load geo /
      placeholder geo / RESIDENT-swap geo (I3), logged per bake with texName +
      facing. Acceptance: flipping a human's facing logs resident-swap geo events
      (the bug made visible and countable). → the probe sits in `resolve` itself
      (geo answered for an EVER-PACKED stem = the flash class, counted on
      `__geoSwapFlashes` + a console warn); first-load/placeholder never
      increment. ROOT CAUSE found by reading, not flipping: `setSubframe`
      EVICTED the pack on every facing re-registration — the repack window
      answered geo (bytes cached — "not a texture load", exactly as reported).
- [x] Fix: a prim swapping between two resident textures never draws geo — hold
      the old binding until the new one binds in the same bake (F2). Acceptance:
      the probe counts ZERO resident-swap geo events across 50 facing flips; the
      first-load silhouette still draws (capture). → `setSubframe` keeps the OLD
      pack serving (reopens only the size guard); a generation stamp discards an
      in-flight pack cut with an outdated rect (serves once, re-kicks). A minute
      of live npc wander (dozens of flips) → `__geoSwapFlashes = 0`; first-load
      geo path untouched (only the eviction changed).

## P3 — solid walls

- [x] Content: `pathable = false` on the wall tile (F3) + the six-consumer sweep
      (I7: golden, rebuilds, cache-bust). Acceptance: golden diff = the one flag;
      worker/npc/wasm rebuilt and restarted. → golden shows exactly
      `tile wall_smooth` joining the impathable list; worker/npc restarted,
      wasm/webgl rebuilt + cache-busted.
- [x] Drill: a route detours around a wall LINE; a wall-enclosed destination
      refuses (F5 log); the standing pen's interior refuses from outside (I4).
      Acceptance: worker logs + a capture of the detour. → the pen interior
      (105,62) logged the F5 refusal; the (98,67)→(110,62) trip crossed the pen's
      line-of-sight and arrived AROUND it — a 120 ms monitor on every mover's
      auth + render tiles vs the wall def counted ZERO violations.

## P4 — outlines occlude

- [x] OutlineOverlay: stamp ALL of the selected object's prims into ONE mask
      (sampling exactly as the draw — I5), edge-detect the UNION (F4). Acceptance:
      the neck test — a selected two-part human shows one ring hugging the
      composite silhouette; no line crosses the neck (capture vs the user's
      screenshot). → one item per OBJECT (union bbox quad, ≤4 parts — the
      primitive-graph law), the frag lights only outside-ALL + adjacent-to-ANY
      (per-part frames/rects/flips, ES 3.00 constant-indexed samplers); captured:
      the ring hugs head+body, the neck is clean.
- [x] Regression: single-prim outlines (wolf, tile squares) unchanged; multi-select
      outlines each object's own union. Acceptance: captures of wolf + a
      two-object selection. → the wolf's silhouette ring unchanged (captured);
      tiles keep box mode; a surface-less placeholder falls back to a box ring
      on its carrier.

## P5 — the vanishing wolf, and the verdict

- [x] The reproduction harness (F5/I6): scripted long cross-seam trips + the gif
      recorder + a per-frame probe logging skip-bake / zero-area subframe / prim
      drop / cull for every mover. Acceptance: the harness runs and the probe log
      indexes by frame. → the 100 ms probe (missing prims / zero-area / lost
      texName / mover GONE) over every mover + 45s alternating cross-seam trips
      + a 9-spot camera-churn pan loop; timestamped log on
      `window.__vanishLog` (the gif recorder proved unnecessary — the probe is
      the sharper instrument).
- [x] Hunt the vanish: correlate every observed disappearance against the probe;
      record the CONDITIONS in issues.md with captures; fix if the cause is cheap,
      else name the successor. Acceptance: issues.md holds either the identified
      conditions + evidence, or the honest no-repro record with what was tried.
      → I11: ZERO events across 6+ minutes — prim loss / zero-size / texName
      loss / zone-close drops ELIMINATED; the STRONG candidate is P2's root
      cause itself (the continuous pre-fix subframe eviction churn, I9 —
      intermittent geo/misdraw DURING movement is the report's exact shape),
      plausibly cured by the P2 fix; the re-armable probe snippet is in the
      issue for re-observation.
- [x] Docs + memory truth pass + a stack bounce with arcs green; **the user's eyes
      close the stream**. Acceptance: docs-check green; captures + logs in
      completed.md. → bug-sweep-delivered memory + MEMORY.md line; docs-check
      green; full bounce: worker composing, the wolf re-adopted, 3 bunny
      adoptions. B1 = the user's eyes.
