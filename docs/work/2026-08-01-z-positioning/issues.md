# Issues — z positioning

_Problems hit, candidate solutions, which we chose and why. **Open issues only** — solved ones are
removed once their resolution is logged in [`completed.md`](completed.md)._

## I1 — Lights have no height, so the caster height test proves nothing {#i1}

**Found while checking whether the head change was already possible; it is my own bug, from
`2026-07-31-lighting-rework`.**

`placeLights()` and `buildRecords()` both call `writePrim` without `unitZ`, so it defaults to 0 for
every light. The gather then reads:

```glsl
float Lz = float(lrec.y >> 24);        // 0, always
...
return Lz * (1.0 - t) <= H;            // 0 <= H  -- true for every caster
```

**The height test is not discriminating at all.** Any caster whose card the ray crosses occludes,
regardless of how tall it is or how high the light sits. That is very likely feeding
[lighting-rework I13](../2026-07-31-lighting-rework/issues.md#i13) defect 3 ("shadows aren't anchored
at the base of our billboards"), because with no light height there is no geometry to anchor to.

**It blocks this whole stream, not just its own fix**: nothing about elevation can be *verified* until
a light has a height for elevation to be measured against. Hence P0.

## I2 — `z` already means draw order in this code {#i2}

Four existing `z`s, none of them height:

| symbol | means |
|---|---|
| `slotZ()` | the painter's key: `PAWN_Z_BASE + zRow + facingDepth·SLOT_DEPTH_Z + i·SLOT_ORDER_Z` |
| `MoverPart.depth` | *"draw-order offset along the view's depth axis"* |
| `prim.zIndex` | sprite ordering |
| `zdepth-world` | the composite carrying that key |

Introducing a fifth `z` that means **height** is how someone later reads `slotZ` as elevation. The
stream says `elevation` upstream of the record ([F1](forks.md#f1)) and P4 renames the draw-order ones,
so the collision closes from both ends.

## I3 — "Where does it stand" is one question, asked in two broken places {#i3}

[lighting-rework I13](../2026-07-31-lighting-rework/issues.md#i13) defect 4: the blit samples the
lightmap at `vWorld = aPosition`, so a billboard takes the light of the ground it is **drawn** over
rather than the tile it **stands** on. Up to a full tile of error at `SQUARE` 128, scaling with sprite
size.

That is the same question this stream is answering for shadows — *what is this object's ground
position, as distinct from where it is drawn* — so the two are fixed together in P4 rather than
separately. Elevation makes the distinction explicit for the first time: before it, "drawn position"
and "ground position" were the same field, and there was no way to write the fix.

## I4 — I do not actually know the projection constant {#i4}

P2 says "derive the drawn offset from `worldTiltDeg`". **I wrote that without knowing which function
of the tilt it is**, and the codebase offers two plausible ones already in use:

| existing constant | value at 65° | what it is used for today |
|---|---|---|
| `elevK = sin(tilt)` | 0.906 | the receiver-elevation gain — shadows climbing a billboard |
| `nsInv = 1 / cos(tilt)` | 2.366 | un-foreshortening N–S distances |

My "sanity check" in the README — that `head.offset.y = -0.87` implies a head height near 1 tile —
is **circular**: I inferred the height from the offset using the constant I was trying to confirm. It
is consistent with `sin`, and that is all it is.

**P2 must derive this from the projection, not fit it to the current offset.** Fitting reproduces
today's drawing exactly and would hide the error in the shadow, which is the one thing this stream
cannot afford — the whole point is that the drawn position and the shadow position stop being tuned
independently. The test that settles it: place a caster at a known height under a light at a known
height and check the shadow length against the geometry, *not* against the old sprite position.

## I5 — `u8 unit.z` is coarser than the offset it replaces {#i5}

`head.offset.y` is `-0.87` **tiles**, a float. `unit.z` is a `u8` in **units** — 16 per tile. So the
head's height quantises to 1/16 tile, and there is no `fine.z` to recover the remainder the way
`fine.x`/`fine.y` do for horizontal placement.

`0.87 × 16 = 13.92` → stores as 14 → **0.875 tiles**. That is a 0.5-unit error, **4 px at `SQUARE`
128**, and it moves the head. Whether 4 px is visible on a pawn's head is a judgement to make *by
looking at it at zoom*, not by arguing from the number — but it must be looked at, because the change
is supposed to be visually neutral.

If it is visible, the options are a `fine.z` nibble (there are 4 reserved bits in `prim_data.GREEN`),
or accepting the snap and re-tuning the art. Not a decision to take before P3 shows the picture.

## I6 — Two coincident casters, one stored identity {#i6}

**The architectural one.** The gather stores **one** caster per (unit, light). Put the head at the
body's x/y and they become two casters with the *same plan-view footprint* — collinear cards at
different heights, competing for one slot.

Whichever the walk finds first wins and the other is simply lost. So a pawn would cast either the
body's shadow (too short) or the head's (a band floating where the body's should be) — not the union,
which is what a viewer expects.

This is not caused by elevation; elevation **exposes** it, because before this change the two parts
had different footprints and never contended.

Three ways out, and it is a design decision rather than a bug fix:

1. **The carrier casts for the whole graph** — the pawn is one caster whose card spans base to the
   top of its tallest part. Parts set `cast_type 0`. Cheapest, and matches "a pawn is one object".
2. **Union at the card level** — keep per-part casters but let the stored identity resolve to the
   carrier, so the card is the composite's extent.
3. **More than one caster per slot** — the storage question that killed the previous design. Not this.

**(1) is the recommendation**, and it wants deciding before P3 rather than discovered during it.
