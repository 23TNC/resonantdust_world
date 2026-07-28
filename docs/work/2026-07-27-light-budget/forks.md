# Forks — light budget

_Decision points, options, which we chose and why._

## F1 — Budget in tiles, or in tile-light pairs? {#f1}

The user's figure was "512". The unit matters more than the number.

- **(a) Tiles.** Simple, matches the existing per-tile dirty texture directly.
- **(b) Tile-LIGHT pairs.** Admission still decided per tile, but the allowance is *spent* in pairs.

**Chosen: (b).** Cost is flat per pair (~0.0015 ms across the whole reach sweep) and a tile carries up to
**16** lights (`PRES_SLOTS = TILE_SLOTS * 2`). So "512 tiles" is anywhere from 512 to 8 192 pairs —
0.9 ms to 14 ms. That is a range, not a budget, and the wide end is exactly the unbounded draw this
stream exists to prevent.

(a) is not wrong so much as unpredictable: it bounds the *count* of the thing whose cost varies, rather
than the thing whose cost is constant.

## F2 — Preventing cold starvation: ageing, or a reserved quota? {#f2}

Hot-first with no correction starves cold lights indefinitely — a continuously orbiting hot light can
monopolise the allowance forever.

- **(a) Ageing.** A deferred tile's priority rises each frame it waits.
- **(b) Reserved quota.** A fixed share of the allowance (say 25 %) is reserved for cold work.
- **(c) Round-robin between classes.**

**Chosen: (a), primary.** Ageing gives a *provable* bound — a tile's priority rises without limit, so it
is admitted within a computable number of frames whatever else arrives. (b) only bounds cold work when
cold work is scarce: with more cold tiles than the reserved share, the cold set still starves internally,
and the split is a tuning knob with no principled value. (c) has the same defect as (b) plus a worse
ordering.

The user's constraint was _"priority to hot lights **without starving cold lights**"_ — starving is the
word that decides it. Ageing is the option that makes non-starvation a property rather than a hope.

**(b) is kept in reserve** as a cheap floor if ageing turns out to make hot lights visibly laggy under
mixed load — the two compose.

## F3 — What a deferred tile shows meanwhile {#f3}

This is where lighting differs from the G-buffer, and the difference is easy to miss.

`SquareCache.bakeDirty` defers *detail*: a tile not yet baked shows older, coarser content. Shadow-cold
is **accumulated**, so a deferred tile shows the PREVIOUS frame's lighting — a shadow in the wrong place,
not a blurrier one. Under motion that reads as a glitch rather than as loading.

- **(a) Show stale.** Simplest; the tile keeps its last value until admitted.
- **(b) Clear to unlit.** Deferred tiles go dark until recomputed.
- **(c) Predict** — offset the stale value by the light's movement.

**Chosen: (a), with the staleness MEASURED rather than assumed** (P5). (b) is worse in every case: a
black tile is more conspicuous than a slightly-late shadow, and it flickers as tiles enter and leave the
queue. (c) is speculative and would need its own correctness story.

The mitigation is the drain order, not the fallback: if the highest-priority tiles are always the ones
nearest the camera and touched by hot lights, the tiles that go stale are the ones least likely to be
looked at.

## F4 — Rationing the dirty set vs SHRINKING it {#f4}

**Out of scope here, deliberately, and the larger win.**

When a light moves 3 px, every tile in its reach genuinely changes — falloff and shadow positions both —
so the 404-tile dirty set is not wrong. But most of that change is **below what the 8-bit lightmap can
represent**: far from the light the falloff is nearly flat and the per-tile delta rounds to zero.
Thresholding the delta would *collapse* the set instead of queueing it, attacking the reach² term that
drives the count in the first place.

Why it is not this stream: a budget is what stops the crash, and it stops it unconditionally. A threshold
is an optimisation whose benefit depends on content and motion, and it needs its own correctness argument
(a delta below the quantisation step this frame can still accumulate across frames into a visible error —
so it needs error carry, not a naive skip). Budget first; shrink second.
