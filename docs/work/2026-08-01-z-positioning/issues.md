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

## I5 — RESOLVED by a channel relayout (user, 2026-08-01) {#i5}

`u8 unit.z` alone quantises height to 1/16 tile with no sub-unit lane, against today's float
`offset.y`. The user's fix moves lanes rather than accepting the snap:

```
GREEN  u8 unit.z | u4 fine.x | u4 fine.y | u4 fine.z | u8 seed | u4 rotation   = 32
BLUE   u2 cast | u2 receive | u2 emit | u10 intensity | u16 definition_index   = 32

definition_data BLUE   u2 cast | u2 receive | u2 emit | u10 intensity | u16 reserved = 32
```

`u4 layer` pays for `u4 fine.z`. **Verified before agreeing: no shader reads the layer lane.** Its only
uses are the write itself and `presence`'s sort — and that sort reads a value the *caller* supplies,
not one fetched back from the record. It is write-only.

`definition_data` loses `layer`/`seed`/`rotation` to `u16 reserved`: seed and rotation are per-prim
with a definition default, and with layer gone the definition has no use for the lane.

**Range.** `u8` units = 255 units = **15.94 tiles ≈ `ZONE_DIM`**, and `frame.span` caps a prim at 16
tiles — so a prim can be up to **16 tiles cubed**, symmetric in all three axes. `fine.z` is `u4` px
within a unit, matching `fine.x`/`fine.y`, which covers `UNIT` at `SQUARE` 256 exactly.

**One doc line needs updating with it**: *"BLUE and ALPHA are copied from the definition"* stops being
literally true — BLUE's low 16 bits become the prim's own definition index, and `seed`/`rotation` move
to GREEN. The copy rule survives, it just describes different lanes.

## I6 — WITHDRAWN: I was wrong about one caster per slot {#i6}

I claimed that body and head sharing a footprint would contend for one stored caster, so a pawn would
cast one part's shadow and lose the other's. **That is wrong, and the user corrected it.**

The gather stores one caster **per UNIT per light** — not one per pawn. Each unit independently keeps
whichever caster occludes *it*, so a unit shadowed by the body stores the body, a unit shadowed only
by the head stores the head, and **the union is represented naturally** across units. There is no
contention at the level I claimed.

The real residual is much smaller and sub-unit: within a single unit that is *partially* covered by
both, the refine tests only the stored caster's silhouette, so a pixel covered by the other one alone
renders lit. As the user put it — the error is the fraction of a unit partially occluded by one part
and not the other, on ground that is often already shadowed anyway.

**Left as a thing to look at during P3, not a design change before it.** I escalated a sub-unit
sampling artefact into an architectural blocker by reasoning about the storage instead of about what a
unit actually does.
