# Issues — map compatibility

_Bugs, gotchas, post-mortems, and the P0/P1 findings. Chronological._

---

## I-1 · The on-prim zoom drift — what's VERIFIED vs the leading hypothesis (P1 must confirm) {#i1}
**2026-07-24.** The reverted shadows-on-prims gather (a `textile_unit` pass, 16 px/tile, contiguous) sampled
the `zdepth` **composite** (a `textile_slot` atlas) by recomputing a UV from the world tile:
`u = ((mod(wc,cols)+1)·slotPx + fract·slotPx) / fixedCW`.

**VERIFIED (facts):**
- `slotPx = floor(fixedCW/(cols+2))` is a zoom-chosen `floor` — observed **48 → 57** live, same texture width.
- `fixedCW` is a fixed reserve, not `(cols+2)·slotPx` (leftover slack on the atlas edge).
- The composite is an apron-padded slot atlas; a world tile's texel is at `(slot+1)·slotPx`, no `tile·SQUARE`
  relation.
- With the elevation path on, the shadows flickered / changed shape / misplaced on zoom, and it corrupted
  even ground shadows (the is-thing read gates the receiver cull for the whole per-light loop). Reverting
  the client fixed it.

**NOT yet proven — the leading hypotheses P1 must decide between (do NOT treat as settled):**
- (a) My recomputed UV mismatches the cache's. NOTE: on paper it *matches* `fillDisplay`'s
  `((sx+1)+frac)·slotPx/cw` — so a pure steady-state formula error is NOT obviously it. Needs a numeric
  check at two zooms, not assertion.
- (b) **Re-bake transient**: on zoom the composite clears + re-bakes over several frames (`BAKE_BUDGET`);
  the gather samples partially-baked `zdepth` → transient wrong is-thing → flicker.
- (c) **Content/constants lag**: the constants row (`cols`/`slotPx` the gather uses) advances to the new
  partition before the composite content finishes re-baking → mismatch for those frames.
- (d) `depthTex.width` (what I divided by) ≠ the cache's `fixedCW` used in `fillDisplay` — needs a direct
  equality check at the allocation site.

**What IS safe to conclude now** (family-level, independent of the exact mechanism): a `textile_slot` map has
no stable, zoom-independent world→texel formula, so addressing it by recomputed world coordinate is fragile
by construction. The disciplined rule — read a `textile_slot` map ONLY via a cache-supplied per-vertex UV
(the display blit), never a world-coord recompute — holds regardless of which of (a)–(d) bit us. P1 still
pins the exact mechanism so attempt #3's fix is aimed correctly. (Lesson from world-geometry I-3/I-5: do not
document a root cause you reasoned to but did not verify.)

## I-2 · `map-model.md` and the shipped composites disagree {#i2}
**2026-07-24.** [`map-model.md`](../2026-07-22-lighting-rebuild/map-model.md) asserts every map is
`cols·R × rows·R`, contiguous, tile-toroidal, and that `textile_square` = `SQUARE` (64) textiles/tile. The
`SquareCache` composites implement none of that: fixed `fixedCW × fixedCH`, `slotPx = floor(...) ≠ SQUARE`,
apron-padded slot atlas. The doc reads as authoritative but describes only the `textile_unit` / data
families; the `textile_square` family is a different animal. P2 must make ONE of them true — either conform
the composites (F1a) or amend `map-model.md` to document the atlas + the "no world-coord cross-reference"
rule (F1b). Until then, `map-model.md` overstates its coverage.
</content>
