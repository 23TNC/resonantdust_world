# Plan — bug sweep

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions
in [`forks.md`](forks.md) (F#), the anticipated-issue inventory in
[`issues.md`](issues.md) (I#)._

## P0 — the paper

- [ ] VARIABLES.md: the panel Z-ORDER table (F1 — tiers 32/40/48/56, tier-recency
      tie-break, chrome above all, On Top REMOVED) beside the details-panel layout
      notes. Acceptance: docs-check green.

## P1 — panel z-order

- [ ] DomPanel: numeric `zOrder` replaces the named bands — z = zOrder × stride +
      tier recency; `bringToFront` advances its TIER's counter only (F1).
      Acceptance: unit-ish probe — two tiers, four panels: clicks reorder within a
      tier, never across.
- [ ] DELETE On Top: the settings-popup row, `_onTop`, the `ontop` band, the
      storage key (ignored on load — I1). Acceptance: a browser with a stored
      onTop=true loads with the panel in its tier; no On Top row in settings.
- [ ] Assign the user's tiers at every constructor (I8): game view 32, details 40,
      inventory 40, chat 48, build 48, settings 56, debug 56. Acceptance: captures —
      details over game, chat over details, settings over chat; chat vs build ties
      flip by last click.

## P2 — the geo flash

- [ ] Instrument the warm re-bake: a probe distinguishing first-load geo /
      placeholder geo / RESIDENT-swap geo (I3), logged per bake with texName +
      facing. Acceptance: flipping a human's facing logs resident-swap geo events
      (the bug made visible and countable).
- [ ] Fix: a prim swapping between two resident textures never draws geo — hold
      the old binding until the new one binds in the same bake (F2). Acceptance:
      the probe counts ZERO resident-swap geo events across 50 facing flips; the
      first-load silhouette still draws (capture).

## P3 — solid walls

- [ ] Content: `pathable = false` on the wall tile (F3) + the six-consumer sweep
      (I7: golden, rebuilds, cache-bust). Acceptance: golden diff = the one flag;
      worker/npc/wasm rebuilt and restarted.
- [ ] Drill: a route detours around a wall LINE; a wall-enclosed destination
      refuses (F5 log); the standing pen's interior refuses from outside (I4).
      Acceptance: worker logs + a capture of the detour.

## P4 — outlines occlude

- [ ] OutlineOverlay: stamp ALL of the selected object's prims into ONE mask
      (sampling exactly as the draw — I5), edge-detect the UNION (F4). Acceptance:
      the neck test — a selected two-part human shows one ring hugging the
      composite silhouette; no line crosses the neck (capture vs the user's
      screenshot).
- [ ] Regression: single-prim outlines (wolf, tile squares) unchanged; multi-select
      outlines each object's own union. Acceptance: captures of wolf + a
      two-object selection.

## P5 — the vanishing wolf, and the verdict

- [ ] The reproduction harness (F5/I6): scripted long cross-seam trips + the gif
      recorder + a per-frame probe logging skip-bake / zero-area subframe / prim
      drop / cull for every mover. Acceptance: the harness runs and the probe log
      indexes by frame.
- [ ] Hunt the vanish: correlate every observed disappearance against the probe;
      record the CONDITIONS in issues.md with captures; fix if the cause is cheap,
      else name the successor. Acceptance: issues.md holds either the identified
      conditions + evidence, or the honest no-repro record with what was tried.
- [ ] Docs + memory truth pass + a stack bounce with arcs green; **the user's eyes
      close the stream**. Acceptance: docs-check green; captures + logs in
      completed.md.
