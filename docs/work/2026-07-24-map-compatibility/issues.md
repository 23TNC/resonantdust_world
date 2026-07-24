# Issues — map compatibility

_Bugs, gotchas, post-mortems, and the P0/P1 findings. Chronological._

---

## I-1 · Post-mortem: the on-prim zoom drift was a cross-family world-coord lookup {#i1}
**2026-07-24.** The reverted shadows-on-prims gather (a `textile_unit` pass, 16 px/tile, contiguous) sampled
the `zdepth` **composite** (a `textile_square` slot atlas) by recomputing a UV from the world tile:
`u = ((mod(wc,cols)+1)·slotPx + fract·slotPx) / fixedCW`. Every term that varies with zoom is a trap:
- `slotPx = floor(fixedCW/(cols+2))` — a **floor**, and `cols` changes with zoom, so `slotPx` jumps in
  integer steps (observed **48 → 57** live). The gather's own grid stays at a constant 16 px/tile.
- `fixedCW` is a **fixed** reserve, not `(cols+2)·slotPx`, so there is leftover slack on the atlas edge and
  the divisor doesn't match the used region.
- the **apron** (+1 slot) and the slot-atlas packing mean a world tile's texel is at `(slot+1)·slotPx`, not
  `tile·SQUARE` — there is no clean world→texel relation independent of `slotPx`.

Net: the two maps' grids are only aligned when `slotPx` happens to divide evenly — i.e. never guaranteed,
and different at each zoom. So `is-thing` / `base-row` were read from the wrong texel at most zooms, which
(because the read gated the receiver-elevation cull for the WHOLE per-light loop) corrupted even the ground
shadows → flicker / shape-change / misplacement on zoom. This is the same class as the original lighting
bug the [`map-model.md`](../2026-07-22-lighting-rebuild/map-model.md) rewrite fixed for the `textile_unit`
family — it recurred because the composites were never brought onto that model.

**Rule of thumb learned:** a shader may read a map by recomputed world coordinate ONLY if that map is a
contiguous `cols·R × rows·R` toroidal grid (the `textile_unit` / data-texture families). The `SquareCache`
composites are safe to read ONLY with a UV the cache handed out per-vertex (the display blit) — never a
world-coord recompute.

## I-2 · `map-model.md` and the shipped composites disagree {#i2}
**2026-07-24.** [`map-model.md`](../2026-07-22-lighting-rebuild/map-model.md) asserts every map is
`cols·R × rows·R`, contiguous, tile-toroidal, and that `textile_square` = `SQUARE` (64) textiles/tile. The
`SquareCache` composites implement none of that: fixed `fixedCW × fixedCH`, `slotPx = floor(...) ≠ SQUARE`,
apron-padded slot atlas. The doc reads as authoritative but describes only the `textile_unit` / data
families; the `textile_square` family is a different animal. P2 must make ONE of them true — either conform
the composites (F1a) or amend `map-model.md` to document the atlas + the "no world-coord cross-reference"
rule (F1b). Until then, `map-model.md` overstates its coverage.
</content>
