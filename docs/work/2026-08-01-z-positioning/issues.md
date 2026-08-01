# Issues — z positioning

_Problems hit, candidate solutions, which we chose and why. **Open issues only** — solved ones are
removed once their resolution is logged in [`completed.md`](completed.md)._

## I1 — WITHDRAWN: I read stale code; lights DO have height {#i1}

I claimed `unitZ` was never written, so `Lz` was 0 and the caster height test collapsed to `0 <= H`.
**False.** `recordSync.ts:194` writes it from the authored light height:

```ts
unitZ: L ? Math.min(255, Math.round((L.height / SQUARE) * UNITS_PER_TILE)) : 0,
```

And `occludesAt()` does a proper height interpolation along the ray — `h = mix(Lz, targetH, t)`,
against a bottom-aligned card `[0, subH]`, with the silhouette sampled at `(hTop - h) / subH`. There is
also a `cast_type 2` branch handling n/s side-frame casters.

I was describing code I wrote earlier in the same session; `lighting-visual` and
`lighting-correctness` ran after it and rewrote exactly these parts. **I diagnosed from memory instead
of re-reading** — the identical failure I had recorded in
[lighting-rework I13](../2026-07-31-lighting-rework/issues.md#i13) and then repeated twice more in
conversation.

**The stream's premise survives, narrowed.** See [I9](#i9).

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

## I4 — RESOLVED: there is no draw-side constant ([F6](forks.md#f6)) {#i4}

_The user settled the coordinate model across four diagrams. The screen-north shift IS the elevation,
1:1; the tilt lives only in `unit.z`'s decomposition and in the shadow transform. Kept for the
reasoning, since it records why fitting the constant to the current sprite offset would have been the
wrong method — it would have reproduced today's drawing exactly and hidden the error in the shadow._

**Original entry:**


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
tiles — so a prim can be up to **16 tiles cubed**, symmetric in all three axes.

`fine.z` is `u4` px within a unit, matching `fine.x`/`fine.y`. **SQUARE is 128, so `UNIT` is 8 px and
the fine lanes only ever use 0–7 — half the nibble.** They are `u4` for headroom, per the design's own
reasoning: `SQUARE` is documented as *"128px, 64px minimum, 256 maximum"*, and at that maximum a unit
is 16 px and needs the fourth bit. The extra bit is a `SQUARE` dial, not a current requirement.

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

## I7 — RESOLVED, and worse than a wrong value: the tilt DOES NOT EXIST {#i7}

I asked whether the angle was the diagrams' **55°** or the code's **`worldTiltDeg = 65`**. The user:
*"whatever use 65 then. We changed to 55 at one point, apparently you changed back."*

**Neither. Grepped: `worldTiltDeg`, `__tilt()` and the `uTilt` uniform do not exist.** They lived in
`shadowGather.ts`, which `2026-07-31-lighting-strip` P2 deleted — I removed the dial with the system
and then quoted 65 from memory of the old code. My own planning language ("off the live tilt",
"`__tilt()` keeps re-tilting coherently") describes machinery I had already deleted.

**This explains the shadow bug more precisely than "the transform is missing".** `occludes()` has no
angle term *anywhere* — it is not using the wrong tilt, there is nothing for it to use. The transform
was never written because the parameter it needs went with the strip.

**Resolution: the value is 65°, and it is reintroduced as a live parameter, not a literal.** The
user's framing is the operative one — *"regardless of world angle our math works"* — so the angle is a
runtime input to the transform, and 65 is just today's setting. It needs:

- one authoritative source (a constant plus a `__tilt()`-style dial), since it is now consumed by the
  record writer (`unit.z`'s decomposition) **and** the shadow transform
- both to read the same source, or the sprite and its shadow disagree about the world — the failure
  this stream exists to remove

A note for whoever re-adds it: the old dial also re-derived `elevK = sin`, `nsInv = 1/cos` and the
normal pitch together, so the whole geometry re-tilted as one. Anything less than that is a partial
dial that lies when turned.

## I8 — The screen↔world transform for the shadow is still unwritten {#i8}

[F6](forks.md#f6) settles the **record** side: what is stored and how the ground position is
recovered. It does not settle what the **shadow** does with it.

Today `occludes()` computes `h = Lz * (1 - t)` and compares it against `H`, with `t` derived from
screen-space y-distances — screen quantities treated as world ones, with no conversion anywhere. That
is not a tuning error; the transform was never there.

The user's draft is:

```
world.y -= cos(90 - world_angle) * unit.z
world.z += cos(90 - world_angle) * unit.z
world.x  = unit.x
```

**The two coefficients being equal is the open question.** Decomposing one vector onto two axes
normally gives a `sin`/`cos` pair; at 55° that is 0.819 and 0.574, a 43 % difference in the height
term — squarely in the range that renders plausibly and lands wrong. Raised, not assumed: the frame
convention may make them genuinely equal, and that is the user's call.

The same applies to walking up the billboard (`world.z += sin(θ) * height`), which looks like it wants
a `world.y` term too, or the card leans as it climbs.

## I9 — The real gap: heights are ISOTROPIC with ground distance {#i9}

Replaces the wrong diagnosis in [I1](#i1) and [I8](#i8).

`Lz`, `targetH` and `hTop` are all in **units** — the same units as x/y ground distance — and
`h = mix(Lz, targetH, t)` interpolates them as if commensurate. **One unit of height is treated as one
unit of ground distance.**

That is a projection choice, and an internally consistent one, which is why the shadows read as
plausible rather than obviously broken. It is also the thing the user's diagrams identify as wrong: the
ground is foreshortened relative to the screen vertical, so a unit of height and a unit of ground
distance are **not** the same world length. The missing relationship is exactly the cyan line —
`tan(world_angle)`.

**This is a metric correction in the height comparison, not a transform written from nothing.** The
ray maths, the bottom-aligned card, the silhouette sampling and the n/s branch are all already there
and already coherent. What they lack is the scale factor between the two axes they compare.

Which also means the **drawn** side needs nothing: drawing is flat and screen-aligned, with the
obliqueness baked into the art.
