# Forks — subframe at ingest

_Decisions taken, with the reasoning. Cited by items and commits so a choice is never re-litigated
from memory._

## F1 — The DSL authors the subframe in FRACTIONS of the master, not units {#f1}

**Chosen: fractions (`0..1`).**

The alternative is whole units of the span's 16-unit grid — what the deleted lanes held. That is
precisely the encoding that produced [normal-frames I8](../2026-08-02-normal-frames/issues.md#i8):
a rounded rect cannot stay registered with anything continuous.

Fractions are also **lod-independent**. A stem streams at 16/32/64/128 px; a fraction addresses the
same art at every one of them, where a unit count silently means a different number of texels per
lod. `sprite_anchor` and `sprite_scale` are already fractions for the same reason, so this adds no
new convention.

## F2 — The crop is scaled to FILL the quadrant, aspect preserved, positioned by the anchor {#f2}

**Chosen: scale-to-fit preserving aspect, then place by the anchor.**

Three candidates:

| | what | why not |
|---|---|---|
| (a) blit at native px, anchor-positioned | simplest | keeps the letterbox — the margin shrinks but never leaves, so the frame still is not the art |
| (b) stretch to fill the square quadrant | zero waste | **distorts** any non-square subframe; the art is authored at a real aspect |
| (c) **scale-to-fit, aspect preserved, anchored** | max resolution without distortion | the residual margin is on ONE axis only, and it is *known* — the anchor says where the art sits in it |

(c) wins because the quadrant must stay **square and pow2** — `quadN` is the master's own width and
the co-pack, the lod ladder and `ppu = frame.w / (span · 16)` all assume it. Changing that reaches
every consumer; (c) reaches none of them. The residual one-axis margin is not the old problem
returning: the old problem was margin whose extent had to be *recovered* at consumption, and here
the anchor states it.

## F3 — The anchor becomes a RECORD LANE, in the freed GREEN word {#f3}

**Chosen: yes — `u16 anchor.x | u16 anchor.y`, in sixteenths of a unit.**

The def's GREEN word was deliberately left free when the subframe was deleted
([normal-frames I8](../2026-08-02-normal-frames/issues.md#i8)): *"the next thing to want it is a real
ANCHOR lane — one number, written once, read by placement and sampling alike."* This is that thing.

Sixteenths match the prim record's existing fine-position quantisation (`fineX/fineY`, P5 of the
lighting rework), so the anchor is exactly as precise as the position it is compared against — which
is the property the old subframe lacked.

`recordSync.groundRowOf` then reads the anchor instead of `p.y + p.height`, and
[lighting-visual I1](../2026-07-31-lighting-visual/issues.md) closes.

## F4 — West's subframe is DERIVED by mirroring east's, never authored {#f4}

**Chosen: derive.**

The user's rule is "each direction, because each direction is a different texture" — and west
**is not a different texture**. `texture-serving-model`: west is *"the east master, mirrored"*, and
the record path already encodes this as rotation 3 = the plain east fields with the mirror applied at
placement and sample. Two authored numbers describing one image is the failure mode this stream
exists to remove.

So the DSL authors **e / s / n**; the client mirrors east's subframe about the frame centre for west.
If a genuinely distinct west master ever ships, it becomes its own stem and authors its own subframe
— the fallback is a new texture, not a new number for an old one.

## F5 — The subframe SUBSUMES `sprite_scale`'s re-centring, and `sprite_scale` keeps its own job {#f5}

**Chosen: split them; the subframe owns placement, `sprite_scale` owns world extent.**

They currently overlap: `setSpriteScale` scales about the `sprite_anchor` pivot **at this exact
seam**, re-centring the art in its frame. If both applied, the art would be positioned twice.

- **subframe** — *which pixels of the master are the art*, and where they land in the quadrant. A
  property of the image.
- **`sprite_scale`** — *how much of the span's world footprint the art occupies*. A property of the
  world (a 0.5 shrub in a 1-tile frame).

So the ingest draw is computed **once**, from the subframe, and `sprite_scale` multiplies the target
rect rather than performing its own pivot re-centre. One rect, one placement, both dials live.

## F6 — `internal_padding` is DELETED; the subframe subsumes it {#f6}

> "We can likely drop internal_padding and leverage this subframe method to accomplish the same thing
> unifying our code and variables." — user, 2026-08-02

**Chosen: delete it.** A uniform inset is a uniform subframe — `internal_padding = p` (units of the
16-unit cell) is exactly the subframe `(p/16, p/16, 1 − 2p/16, 1 − 2p/16)` in [F1](#f1)'s fractions.
Keeping both would mean two crops applied at one seam, which is [I2](issues.md#i2)'s double-crop and
the same *two-numbers-for-one-thing* failure the whole stream exists to end.

**Per CELL, not per stem, and that needs no new indexing.** The resolver already keys frames by
`(stem, cell)` — `resolve(stem, map, cell)` and `defByKey`'s `stem#cell` — and a linked stem's
"directions" are its autotile cells the way a sprite's are its facings. So [F4](#f4)'s per-direction
rule generalises to per-cell without a second mechanism: one subframe per addressable frame.

A grid stem authoring ONE subframe applies it to every cell, which reproduces `internal_padding`
exactly. Authoring per cell is then a capability the pad never had, for a grid whose cells are not
uniformly inset.

**Consequence:** grid stems stop being a special case. `packCoPack`'s `entry.grid` exclusion for
`sprite_scale` should be re-examined in the same pass ([F5](#f5) splits placement from world extent,
and the reason grids were excluded was the placement half).

## F7 — There is NO new anchor variable; `sprite_anchor` is re-based onto the subframe {#f7}

**Chosen: reuse `sprite_anchor`.** The plan originally added `&thing.anchor_point.x/y`. It should
not, because the variable already exists and already means this.

`sprite_anchor` is documented as *"the pivot ON THE SPRITE that aligns to `anchor`"*, and
`TextureResolver.setSpriteScale` states its frame explicitly: *"fractions of the sprite's **opaque
bbox**"*. The opaque bbox is precisely what the authored subframe replaces — so `sprite_anchor` was
**already** fractions-of-the-subframe, measured against a *derived* subframe.

So its meaning does not change at all; it becomes **exact**. The frame it is measured against stops
being recovered from pixels (and stops drifting with streaming order, [I7](issues.md#i7)) and starts
being authored. Adding `anchor_point` beside it would have created two pivots on one image — the
same two-numbers-for-one-thing failure as [F6](#f6) and [normal-frames I8](../2026-08-02-normal-frames/issues.md#i8).

**Per direction, like the subframe** ([F4](#f4)): each facing's subframe differs, so the feet sit at
a different fraction of each. `&thing.sprite_anchor.<e|s|n>.x/y` overrides the non-directional
default the same way `&thing.subframe.<dir>` does.

## F8 — Subframes are measured from the MASTERS ON DISK, never from the client {#f8}

**Chosen: measure the masters.** [I7](issues.md#i7) killed the alternative: the client's
`computeSpriteBBox` runs on whichever lod decoded first, so the same art reported `fy = 0.125` in one
session and `0.109` in the next. Authoring a permanent number from a measurement that moves with
streaming order would bake in the exact class of error this stream exists to end.

The masters under `textures/` are the ground truth and carry no lod ambiguity — `meta.json` states
`"square": 128` and every map is that size. Measuring there gives one answer, reproducibly.

**The bbox is taken from the SURFACE map's B channel**, matching what the client derived from
(`surface = R presence / G ao / B alpha`), so the authored number describes the same silhouette the
renderer will silhouette-test against.

**A kind with several variants takes the UNION of their bboxes.** The DSL authors per kind ×
direction, but `flora` has 13 variant folders sharing one stem. A per-variant crop would need
per-variant authoring; the union is the conservative choice — it never clips art off any variant, and
it costs only the margin where variants disagree. If a variant is ever far tighter than its siblings,
[F6](#f6)'s per-cell capability is where that gets fixed, not here.

## F9 — ONE 0..15 rotation index space, covering facings AND linked cells {#f9}

> "You are almost certainly going to need rotation/direction 0..15 because linked directions are
> 0..15. That would generalize our rotation/direction lookups." — user, 2026-08-02

**Chosen: index 0..15, with `s/e/n/w` as readable aliases for 0/1/2/3.** This supersedes the
three-entry `[e, s, n]` array [F4](#f4) and [F6](#f6) were written against.

The evidence that this is the right shape was already in the record layout: **`ROTATIONS_PER_DEF` is
16** — `definition_data` is *"16 sequential px per definition, indexed by ROTATION"*. The def has
always had sixteen slots per definition. A sprite uses four of them (`FACING_BY_ROTATION`:
`0 = s, 1 = e, 2 = n, 3 = w`) and a linked tile uses all sixteen (`build-walls`: the autotile cell is
`y·4 + x`, `x = N+2E`, `y = 3−(S+2W)`). They were never two index spaces — one was a prefix of the
other, and treating them as different is what forced `internal_padding` to exist as a parallel
mechanism in the first place.

So a subframe is authored **per rotation index**, and "direction" and "autotile cell" stop being
different lookups:

```
&thing.subframe.0.x   … .15.h        by index — the general form
&thing.subframe.s|e|n|w.x            aliases for 0|1|2|3, so sprite corpora stay readable
&thing.subframe.x                    non-directional: the fallback for every index
```

**West stays derived, and the aliases make that visible rather than implicit** ([F4](#f4)): index 3
resolves to the east master plus `flipX`, so authoring `subframe.w` is authoring a rect for an image
that does not exist. The alias exists so the fallback chain reads uniformly, not so west gets its own
art.

**This is why [F6](#f6) lands for free.** A linked stem's per-cell subframe is just indices 0..15 of
the same array, so `internal_padding` retires into it with no per-cell mechanism of its own.
