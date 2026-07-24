# Forks — map compatibility

_Decision points + options + which we chose + why. Chronological._

---

## F1 · Conform the composites to the map-model, or forbid cross-referencing them? {#f1}
**2026-07-24 — OPEN (the P2 decision; the whole stream feeds it).** The `SquareCache` composites are a
**`textile_slot`** family (one variable-`slotPx` slot per tile in a fixed apron atlas), NOT the contiguous
`textile_square` [`map-model.md`](../2026-07-22-lighting-rebuild/map-model.md) claims. **Either way, formalise
`textile_slot` in `squareMath.ts` + `map-model.md`** so the family is named and its addressing rule is
documented. Then choose how the composites live:

- **(a) Conform the composites to the model.** Store each composite as a contiguous, un-padded
  `cols·SQUARE × rows·SQUARE` toroidal texture (same shape as `shadow-cold`, just at `SQUARE` res). Then a
  world tile resolves to the same cell in EVERY map and any shader may look up any map by world coord at any
  zoom — cross-family lookups become legal, on-prim casting's original approach just works.
  *Cost:* re-work the cache's allocation, the wrap-apron seam handling, the bake target, and the display
  UVs; risk to the working blit; memory grows if `SQUARE` res is kept at all zooms (the fixed atlas exists
  partly to CAP texture size — `RESERVE_CSS` — and to let `slotPx` shrink when zoomed out).
- **(b) Keep the atlas; make cross-referencing a composite by world coord ILLEGAL.** Rule: a composite may
  only be read with a UV the cache itself supplied per-vertex (the display blit), never a UV recomputed from
  a world position in an unrelated pass. Any feature that needs per-pixel composite-like data in another
  pass derives it **within that pass's own family** — e.g. on-prim casting reads the caster **buckets**
  (already in the `textile_tile` data texture, zoom-safe) to learn the receiver prim + its base row, or a
  `textile_unit` depth is baked alongside `shadow-cold`.
  *Cost:* each such feature does a little more work in-family; *benefit:* zero change to the working render
  path, and the fixed-atlas size cap + zoom LOD stay intact.

**Lean (b)** — it's lower risk to the shipped lighting and preserves the fixed-size cap that the atlas
exists for, and on-prim casting has a clean in-family source (the caster buckets already tell the gather
which prims stand where). (a) is the "purer" model but a large, risky re-plumb whose main beneficiary is
exactly the lookup we can avoid. Decide in P2 after the P0 inventory shows whether anything ELSE needs
cross-family composite reads.
</content>
