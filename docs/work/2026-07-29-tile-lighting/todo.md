# Todo — tile-lighting

_Items tick in place; the box is the move. Design: [`README`](README.md) (D1–D4). Visual
items verify at ≥2 zooms, tab VISIBLE; cold-cost claims verify via `__bakes()` counters._

---

## P0 · The audit (find, don't assume)

- [x] Pin WHERE tile prims fall out of the caster/receiver pipeline today (`buildCasters`
      walks every standing cold prim; the wall def resolves yet no record mints), and what
      a naive record WOULD do (leaning-card shadow from a floor tile?). Acceptance: the
      mechanism named in `completed.md` with file:line; D1's caster-geometry call resolved
      as a fork.
- [x] The participation gate (D2): decide + record how a tile opts in (resolved normal
      map? a content lane like `&tile.height`? both), keeping `white` ground bit-identical.
      Acceptance: the gate written in `forks.md`; grass provably unaffected (counter probe
      unchanged on a fresh zone).

## P1 · Tiles in the COLD class

- [ ] Tile records: participating tiles mint receiver (+ caster when walled) records in
      the COLD class with the P0-decided geometry; the tile's atlas NORMAL feeds per-light
      N·L at its texels (replacing ground's ndl=1 there). Acceptance: a torch-lit wall
      shows directional shading across its faces (capture ≥2 zooms).
- [ ] Casting: a wall throws a cold shadow with the P0-decided card; height 0 (flat tiles)
      never casts. Acceptance: the wall ring near a torch casts a coherent shadow; grass
      casts nothing.
- [ ] Change cadence (D4): a BUILD re-bakes only the affected cold rects (cells + reaching
      lights), once; walks/drags after it bake ZERO cold. Acceptance: `__bakes()` deltas
      across a build + a 10 s walk logged in `completed.md`.

## P2 · The blueprint in the HOT class

- [ ] Preview tiles register HOT records while the preview lives (D3 — the mover idiom:
      records + hot rects per drag move, dropped on release/exit; hot texels re-bake
      clean). Acceptance: dragging near a torch shows the blueprint lit/shadowed; release
      leaves no residue (hot map probe); cold bakes 0 for the whole drag.

## P3 · The drill

- [ ] End-to-end: build a wall ring beside a torch — blueprint lit while dragging (hot),
      built walls shade + cast (cold, one bake burst), reload-stable, 120 fps at zoom 1.
      Acceptance: numbers + captures in `completed.md`; the user's eyes are the final
      oracle.
