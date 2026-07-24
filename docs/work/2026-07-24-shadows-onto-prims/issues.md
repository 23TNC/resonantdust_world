# Issues — cast shadows onto prims

_Bugs, gotchas, and inherited lessons. Chronological._

---

## I-4 · The receiver mask had to be hand-aligned to the drawn sprite (NW ~0.5u W, ~2u N) {#i4}
**2026-07-24 — applied, not root-caused.** The receiver mask (`receiverCover` in `shadowGather.ts`) decides
"is a prim drawn at this texel" from the **def's tight-bbox anchor** `Ac` (built by `coldShadowData` off
`prim.y+height` + the def offset/`sh`). That anchor does **not** land exactly where the sprite is actually
DRAWN by `thingPlacement.placeThing` — so the sprite-shaped shadow "cut" came out offset from the albedo:
~**2 units too far SOUTH** and ~**0.5 unit too far EAST**. We correct it by shifting the mask **north-west**
by `RECV_ALIGN_X = 0.5` / `RECV_ALIGN_Y = 2.0` (`Ac -= vec2(0.5, 2.0)`), found by eye against the albedo.

- This is a **cut alignment**, NOT a shadow-field shift — `SHADOW_LIFT` (the ground-shadow seat) stays 3;
  moving it doesn't help and was a red herring.
- **Root cause not chased:** the discrepancy is between the tight-bbox anchoring in `coldShadowData` and the
  draw placement in `thingPlacement` (candidates: the tight-bbox `oy`/`H`/`spanU` math vs the sprite pivot
  `sy`, an even-unit bbox rounding for the 0.5 x, and/or the full-box-bottom `prim.y+height` vs the visible
  opaque base). A clean fix would derive the mask from the SAME placement the draw uses, retiring the two
  constants. Deferred to P4 polish; the constants are the one knob if it ever drifts.

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
