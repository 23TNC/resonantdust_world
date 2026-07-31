# Issues — pawn part placement

_Problems hit, and what the evidence actually showed. Findings recorded at plan time are marked as
such — they were read out of the code, not measured under this stream._

## I6 — The head renders TWICE at two scales — cold at 128 px, warm at 80 px {#i6}
_2026-07-30 · P0.1 · **measured live, and this is the user's "head lighting isn't scaled"**_

At game zoom 2 the pawn shows **two heads**: a large pale one up-left carrying real facial detail
(ear, profile — so it is TEXTURED, not a geo placeholder), and the correct smaller brown one inside
the selection outline.

The mover layer holds exactly **two** prims and no duplicate:

```
id 4  body  x 13312  y 6528  w 128   pawn/human/female/7/e
id 5  head  x 13336  y 6518  w  80   pawn/human/female/11/e.1
```

So the oversized head is **not** a second warm prim — it must come from the COLD layer, i.e. the
head is present in both cold and warm with DIFFERENT geometry (128 vs 80). The blit composes them
as `alb = mix(cold, warm, wcov)`, so wherever warm coverage is 0 the cold head shows through — a
head-shaped ring around the correctly-scaled one, lit by the cold path only. That is exactly the
"lighting not scaled" symptom: the lighting is right for the geometry it was baked against, and
there are two geometries.

**This reframes the stream.** [I1](#i1) (lightmap sampled at the drawn position) is real but is a
sub-tile offset; this is a whole duplicate at 1.6x scale and dominates the visual. It also explains
why the user saw "normal scaled, albedo did not" — two composited copies at different scales read
exactly that way, and [I2](#i2) was already refuted as impossible at the bake.

**ROOT-CAUSED 2026-07-30.** Neither cold-layer candidate was right — there is no cold `pawn/human`
prim at all (`__viewport.map.prims` holds none), and the warm cache holds exactly the wolf, the
cursor light and the pawn's two parts. Removing the head prim removed BOTH heads; the
`albedo-warm` overlay shows only ONE head, at the correct 80 px. So the ghost is not albedo — it is
the **lighting silhouette**, and it is 1.6× because the shadow/light CARD is sized from the def,
not from the prim:

```js
// coldShadowData.definitionFor
const stRaw = Math.max(1, Math.round(Math.max(billboard.width, billboard.height) / SQUARE));
let st = 1; while (st < stRaw) st <<= 1;   // pow2 TILES
const spanU = st * 16;                     // → the tight box, in world px
```

The head draws at `128 × 0.625 = 80` px. `round(80/128) = 1`, so the card is **one tile = 128 px**
and every downstream world extent (`defTight`, the silhouette quad, the lightmap deposit) is
`128/80 = 1.6×` the drawn sprite. Measured against the art: the head's opaque bbox is
`0.75 × 0.875` of its frame, so the card is 96×112 world px while the drawn head's art is 60 px
wide — 1.6× exactly, concentric, which is the ghost.

**Proved live**: growing the head prim to 128 px (its def's frame span) collapsed the two heads
into one correctly-lit head. Nothing else changed.

**Why it cannot be fixed by scaling the def.** `ppu = 2^lod / spanU` must be a whole pow2 — the
whole-px-per-unit invariant in [`VARIABLES.md`](../../VARIABLES.md) §definition_data. A card of
`80 px = 10 units` gives `ppu = 12.8`, so the def drops to the loose lod-0 fallback. The def model
**requires the drawn world span to be a pow2 tile count**; a free-form draw-time multiplier can
never be expressed in it. Resolved as [F4](forks.md#f4): the per-slot size moves to `sprite_scale`,
where the art — and with it the bbox, the tight box and all four co-packed maps — scales together.

## I7 — `sprite_scale` never reaches a multi-facing stem {#i7}
_2026-07-30 · found while root-causing [I6](#i6) · **live**, out of this stream's scope_

`WorldBridge` registers a kind's authored `sprite_scale` under the kind's BASE stem:

```ts
const stem = textureNameFor(this.thingStems[kind - 1]);   // "pawn/animal/wolf"
this.resolver.setSpriteScale(stem, l.scw, l.sch);
```

but the resolver looks the scale up by the RESOLVED texture name at pack time
(`pawn/animal/wolf/e` — facing appended). Read back live, `spriteScale` holds exactly two entries,
`biome-thing/default/flora → 0.5` and `pawn/animal/wolf → 1.125`, and neither key is ever a name
that gets packed. **So every authored `sprite_scale` in the corpus is currently inert.**

Deliberately NOT fixed here. Honouring the two live values would resize the wolf and every flora
instance — a visible change to two kinds this stream was not asked to touch. [F4](forks.md#f4)
therefore registers pawn slot scales under their EXACT resolved names, which needs no lookup
change; making the base-stem registration work is its own piece of work.

## I8 — `maxCardTiles` walks the layout table on a stale stride {#i8}
_2026-07-30 · noticed while removing `body.size` · **live**, out of scope_

```ts
for (let kind = 1; kind * 7 <= this.thingLayout.length; kind++)   // WorldBridge.ts:265
```

`LAYOUT_STRIDE` is **10**, not 7, so the loop runs ~1.4× the real kind count and the tail iterations
read past the table — `readLayout` returns `DEFAULT_LAYOUT` for them, so the max is unaffected today
and nothing is corrupted. It reads `size`, which [I4](#i4) showed is inert for the drawn box; the
live `maxTightHpx` is the real bound anyway. Left alone: touching the shadow-walk dilation bound is
not this stream's business.

## I4 — `body.size` never reaches the drawn box; `placeThing` uses `span` {#i4}
_2026-07-30 · P0.3 · **measured live** — invalidates [F3](forks.md#f3)_

Read back from a live human (`__moverLayer`, mover 813694981, facing north):

```
p0 body:  x 13696  y 6656.12  w 128  zIndex 53.0009  tex pawn/human/female/7/n
p1 head:  x 13720  y 6645.98  w  80  zIndex 53.0109  tex pawn/human/female/11/n.1
```

- **Head w = 80 = 128 × 0.625.** `head.scale` IS applied — that half of the skeleton works.
- **Body w = 128 = exactly one tile**, while `pawns.rd` authors `1.5 &body.size set`. The box
  comes from `placeThing`, which computes `drawW = l.span * (scaleAtDraw ? l.scw : 1)` — off
  **`span`**, not `size`. `span` is 1, so the body draws at one tile and `body.size` is inert for
  the drawn box.

**This breaks [F3](forks.md#f3).** The plan said the 0.8/0.5 request is one edit — `body.size`
1.5 → 0.8 — with the head following by the 0.625 relative scale. But changing an ignored field
changes nothing. The body's drawn size is `span × SQUARE`, and `span` is pow2 tiles, so 0.8 is not
expressible there at all.

So the request needs a real decision about WHICH field sizes a part, not a value edit. F3 must be
re-resolved before [P1](todo.md) can run.

**Seating, for the same reason:** the head's top sits only 10.1 px above the body's top while
being 80 px tall, so it is almost entirely inside the 128 px body — which is exactly what the
before-image shows. `head.offset.y -1.15` tiles should be ~147 px at SQUARE 128; the observed
separation is 10 px, so the offset is being scaled by something other than SQUARE (MoverLayer
derives `tilePx = size0 / slots[0].size`, i.e. it divides by the very `size` that does not size
the box).

## I5 — Pawn textures do not load on reload until the pawn moves {#i5}
_2026-07-30 · reported by the user during P0 · not yet root-caused_

On a fresh page load a pawn renders without its textures; moving it makes them appear. Recorded
here because it costs a reload-and-nudge on every visual check in this stream, and because a
texture that resolves only after a move is a plausible contributor to the misalignment reports
that opened this stream. Not investigated yet — [P0](todo.md)'s captures work around it by moving
the pawn first.

## I1 — The blit samples the lightmap where a sprite is DRAWN, not where it stands {#i1}
_2026-07-30 · read at plan time from `albedoBlitShader.ts`_

```glsl
vWorld = aPosition;                 // world px of this screen pixel
ivec2 lt = lightTexel(vWorld);
```

Every fragment reads the lightmap at the ground point it is drawn over. A standing billboard is
drawn UP-SCREEN of its footprint — `anchor.y = 1.0` with `sprite_anchor.y = 1.0` pins the feet to
the cell's front edge and the art rises from there — so its upper pixels sample tiles NORTH of the
tile the pawn occupies.

Estimated at up to a full tile (**128 world px at `SQUARE` 128**) and scaling with `size`, which
means the current `body.size 1.5` makes it worse than the `0.8` this stream moves to. [P0](todo.md)
measures the real figure rather than trusting the estimate.

**Direction is not the problem.** `worldNormal(n, phi)` already pitches a standing billboard's
camera-facing normal into the world frame by `90° − tilt`, and ground stays unpitched. So the
shading DIRECTION accounts for standing geometry; only the sample POSITION does not.

Resolved as [F2](forks.md#f2): recover the footprint row from `zdepth_world.b`, which the blit
already reads for occlusion.

## I2 — "The normal does not scale with the albedo" is unconfirmed and may be impossible {#i2}
_2026-07-30 · **REFUTED at P0.4** — measured; see [completed.md](completed.md)_

The user observed the normal scaling correctly while the albedo did not. That cannot happen at BAKE
time: `SquareCache` bakes *"N sibling CHANNELS (albedo/normal/surface/zdepth) from ONE shared prim
index"* through a merged MRT pass, and `mrtBakeShader` writes all four from a single quad
(`layout(location = 0..3)`). One quad cannot scale one attachment differently from another.

So if the divergence is real it happens EARLIER — in the art pipeline (`normalize_square`) or at
atlas ingest — and the two maps are different sizes on disk or in the atlas, which would make each
channel sample its own frame. That was demonstrably true of `pawn/human` earlier today: `layers`
and `albedo_marigold` sat at 256 while `albedo`/`diffuse`/`normal` were 128.

**That specific instance is now fixed** — every `pawn/human` map is 128×128 with `span 1`,
`square 128`. So the observation may simply predate the fix. [P0](todo.md) compares the two atlas
frame sides before [P4](todo.md) changes anything; if they match, this closes as refuted.

## I3 — Every part slot shares one `zIndex` {#i3}
_2026-07-30 · read at plan time from `MoverLayer.ts`_

```ts
const zIndex = PAWN_Z_BASE + box.zRow;      // computed ONCE from slot 0
```

`box` comes from `placeThing(...)` on slot 0 (the carrier), and the same `zIndex` is then handed to
every `SlotSpec`. There is no per-slot ordering to express, so "head under body when facing north"
has nothing to attach to today. This is why requirement 2 is real work rather than a value change,
and it is the gap [F1](forks.md#f1) fills.

Note the interaction to respect: `placeThing` derives `zRow` by clamping `floor(anchorY)` into the
footprint's rows, and the blit arbitrates warm-over-cold with a wrapping row compare
(`south = (cdb - wdb) & 0x7f`). Per-slot depth must land in a form BOTH of those still read
correctly — [P2](todo.md)'s last item is the check.
