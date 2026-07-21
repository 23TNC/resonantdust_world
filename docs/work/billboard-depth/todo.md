# Todo — billboard-depth (execution order)

_Planned, not started. Items move to [`completed.md`](completed.md) when done + verified. Phases run in
order; each verifiable in-browser on its own. See [`README.md`](README.md) for the architecture and
[`forks.md`](forks.md) for the open decisions. Geo tier throughout (no textures)._

---

## P0 · Billboard W × H for sprite-less things — 2026-07-21

Today `placeThing` builds a **square** box (`side = l.size · SQUARE`, `width == height`), so a conifer draws
squat. Give the billboard a real **width × height** (tiles) so a sprite-less conifer reads as a tall box and
its shadow (which already reads `prim.width/height`) fans proportionately.

- [ ] Decide + source the billboard **W × H** (tiles) per thing def ([F1](forks.md#f1)) — a new
      `&thing.billboard w h` field (authoritative in `docs/VARIABLES.md`/`TABLES.md` if it changes the
      `thing_layout` stride), or derived from `footprint`/`size`. Conifer > 1 tall.
- [ ] Plumb it: the layout decode (`readLayout`, the stride-7 → stride-N table if a field is added),
      `placeThing` → `ThingPlacement.width/height` (no longer forced square), `WorldBridge.onColdThings`
      `addPrim({ width, height })`. Keep bottom-anchoring (`anchor.y = sprite_anchor.y = 1`).
- [ ] Confirm the geo bake places the box at the new W × H (the bake already reads `prim.width/height` via
      `uModel`) and z-sorts by base row unchanged.
- [ ] **Verify** in-browser at `?focus=100,50`: conifers render as tall billboards (not squares); their
      cast shadows are proportionately tall.

## P1 · Write `zdepth-world` properly (tiles + things, toroidal-wrapped) — 2026-07-21

`zdepth-world` currently: things write `uTileDepth`, **ground writes black**. Make it the real ordering key.

- [ ] Write each fragment's **world-tile depth** into `zdepth-world` for **tiles AND things**, encoded as a
      **wrapped** world value (matching the toroidal buffer's wrap, so depth compares across the seam) — the
      "write out tile with the wrap-around". Larger world-Y (nearer the camera-bottom) = drawn-on-top.
- [ ] Decide the encoding/precision (a single byte of wrapped tile-row is likely enough for ordering;
      confirm it survives the toroidal reproject — nearest, no bilinear blur of the depth).
- [ ] **Verify** via `/overlayRT zdepth-world-cold`: a clean per-row step/gradient over the world, stable
      under pan + zoom (no seam discontinuity).

## P2 · Order the shadow pass under things (shadows over tiles, under conifers) — 2026-07-21

- [ ] The shadow pass reads `zdepth-world` at each screen fragment and **suppresses the shadow where a thing
      occludes that ground point** (thing depth nearer than ground), so conifers show on top while shadows
      still land on bare tiles ([F2](forks.md#f2): sample `zdepth` in the cast shader vs reorder passes vs a
      display-time depth test). Requires the shadow pass to sample the world composite in register (it
      already computes each fragment's world pos).
- [ ] **Verify** at `?focus=100,50`: a conifer sitting in front of a shadow shows on top of it; the same
      shadow still darkens the bare ground beside the conifer; pan/zoom keep the ordering across the seam.

## P3 · Retain shadows default-on (provisional) — 2026-07-21

- [ ] Keep the `ShadowCaster` display **on by default** (current behaviour) while shadows are in progress.
      Leave a header/comment marking it provisional — the end state makes `shadow-cold` a **lighting input**
      (gated behind an overlay/lighting toggle), not a direct display. This stream does **not** build the
      lit consumer; it only preserves the debug default.

## P4 · Verify the whole ordering — 2026-07-21

- [ ] In-browser at `?focus=100,50`, geo tier: conifers are tall billboards; their shadows fan
      proportionately; **conifers draw over tiles + shadows, shadows draw over tiles only**; the ordering
      holds under pan + zoom across the toroidal seam; zero console errors. Then hand back to
      [`webgl-engine`](../webgl-engine/todo.md).
