# Issues — shadow-world

_Anticipated gotchas (seeded on open 2026-07-20). Move to "resolved" as they land._

---

## I-1 · Combine feedback loop — still ping-pong — open

The world-space combine is the same read-modify-write as shadow-cast: reading and writing the same RT is a
feedback loop (UB). **Guard:** ping-pong `shadow-a`/`shadow-b` — read `src`, write `dest`, cast through a
separate world `mask` (source ≠ destination). Unchanged from shadow-cast; just over the world buffer.

## I-2 · Zoom-reprojecting a world-space bitfield garbles bits — open

`shadow-a`/`-b` scale with the cache window on zoom; a bitfield can't bilinear-resample. **Guard:**
**nearest** reproject, then re-cast to refill at the new scale (same rule as `shadow-cold` — [shadows
I-1](../shadows/issues.md#i-1)). The move + dirty-queue already re-casts lights over time; a zoom marks all
5 dirty (or clears + re-seeds).

## I-3 · Toroidal write alignment — reuse the cache mapping — open

The world `mask` write + the `/overlayRT` read must both use the **exact** window→slot mapping the
composites use, or shadows drift from the world / from the markers. **Guard:** derive the cast's
world→buffer transform and the overlay's sample from the same `SquareCache` window math ([F3](forks.md#f3),
[shadows I-2](../shadows/issues.md#i-2)). FAIL symptom: shadows offset from their casters, or sliding under
pan.

## I-4 · Camera pan re-dirties nothing but shadows should follow — resolved-by-design — open

Because storage is now **world-space**, a pan does **not** invalidate the shadows — they're already at the
right world position and just re-display through the panned overlay geometry (unlike shadow-cast's
screen-space store, which a pan would have staled). This is the payoff of world-space. (A pan only forces
work if it moves the toroidal window enough to expose un-cast rects — then those lights re-cast.)

## I-5 · Bits are float-mod, A never data — open

RED byte, bits 0–4; A held at 1. Pixi high-shader is ES 1.00 (no `uint`), so set/test/clear bits with
float math (proven by the archived bitfield-rt). Alpha carries no data
([`rendering-platform.md`](../../components/client/webgl/design/rendering-platform.md)).

## I-6 · Overlay must sample the CURRENT ping-pong buffer — open

`/overlayRT shadow-a` names a fixed channel, but the "current" bitfield alternates `a`↔`b` each update.
**Guard:** expose the **current** buffer under a stable name (e.g. `shadow-a` always = the latest), or list
both and let the decode read whichever `renderTextures()` reports as current — so the overlay never shows a
one-update-stale buffer.
