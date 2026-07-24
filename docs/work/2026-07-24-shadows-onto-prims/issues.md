# Issues — cast shadows onto prims

_Bugs, gotchas, and inherited lessons. Chronological._

---

## I-1 · Inherited: the receiver-input lesson (why attempt #2 reverted) {#i1}
**2026-07-24.** Attempt #2's gather math was correct; it reverted because the receiver `is-thing`/`base-row`
came from the `zdepth` **`textile_slot` composite** read by recomputed world coordinate, which is not
zoom-stable (`slotPx` is a zoom-chosen `floor` in a fixed apron atlas). Full root cause + the compatibility
rule: [`2026-07-24-map-compatibility`](../2026-07-24-map-compatibility/README.md) and
[`2026-07-23-shadows-on-prims/completed.md`](../2026-07-23-shadows-on-prims/completed.md). This stream's
entire reason for existing is to feed that same math from a zoom-safe in-family source instead.

## I-2 · Acceptance is a ZOOM SWEEP, not a fixed-zoom screenshot {#i2}
**2026-07-24.** Attempt #2 was declared "verified" off screenshots + an identity check taken at ONE zoom —
and it was wrong on zoom. For this stream, every phase's VERIFY step must exercise ≥3 zoom levels and a live
zoom transition. A clean fixed-zoom frame is necessary but nowhere near sufficient.

## I-3 · Debug gotcha: the render loop is change-gated {#i3}
**2026-07-24.** Freezing every light stops the viewport ticking, so console edits don't bake and a
`debugReadShadow` identity diff compares STALE RTs (trivially 0 mismatches — meaningless). Keep ONE
zero-reach light `dynamic` as a keep-alive; confirm ticks via `__gather.debugDirtyTiles != 0` before
trusting any read.
</content>
