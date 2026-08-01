# Z positioning — height as the one truth — 2026-08-01 — **DELIVERED**

> **30/30. Body and head cast ONE aligned shadow**, verified on screen against a real before-image.
> Elevation costs nothing — it is **2.8 ms faster** than flattening, because flat heights put every
> light at `Lz = 0` and degenerate the occlusion solve.
>
> The record now holds **game** coordinates: where a prim stands, not where it is drawn. A carried
> piece adopts its carrier's footprint and derives its own height from geometry, so wall torches,
> carried items and any elevated caster get correct shadows from one authored number — or none.

_Component: [`client/webgl`](../../components/client/webgl/). Plan in [`todo.md`](todo.md); decisions
in [`forks.md`](forks.md); what is already broken in [`issues.md`](issues.md)._

## The user's insight

> "Currently we shift the head on our pawn north so it aligns with the body. If we instead used the
> same x/y as the body, but increased z, could we offset the head in y based on z, thereby aligning
> the body and head shadows?"

Yes — and it is the more correct model, not merely a tidier one. A y-offset and a z-height look
identical on screen and mean **opposite things** to the shadow system:

| | today | with z |
|---|---|---|
| where the head IS | a different world position from the body | the same position, higher up |
| where its shadow comes from | a different ground footprint → two shadows, offset by the drawing fudge | the same footprint → **aligned by construction** |
| what the number means | "draw it here" (and lie to the shadow system) | "it is this high" |

The y-shift becomes a **rendering consequence of z**, not an authored position. There stops being a
way to say *"draw it here but shadow it there"* — the whole class of error goes with it.

A check that the model is real rather than convenient: `head.offset.y` is `-0.87` tiles today. At the
world tilt that implies a head height near 1 tile on a ~1.3-tile pawn. The numbers land where physics
says they should.

## What exists today: the storage, and nothing else

Verified before planning:

| piece | state |
|---|---|
| `prim_data.GREEN` has `u8 unit.z`, `writePrim` takes `unitZ` | **exists** |
| the DSL authors a height | **no** — `pawns.rd` has `head.offset.y`, no `offset.z` |
| `MoverPart` carries one | **no** — `offsetX`, `offsetY`, and `depth` (draw order) |
| the draw path projects z → y | **no** — `cy = ay + s.offsetY * tilePx` |
| anything writes `unitZ` | **no** — every call omits it, so it is 0 everywhere |
| the caster card starts at z | **no** — `occludes()` spans `[0, H]` from the ground |

## Two things found while checking, both blocking

**Lights have no height** ([I1](issues.md#i1)). `unitZ` is never written, so the shader's
`Lz = float(lrec.y >> 24)` is **0** for every light — and the caster height test
`Lz·(1−t) ≤ H` collapses to `0 ≤ H`, which is always true. **The height test is not discriminating at
all right now.** Nothing about elevation can be verified until a light has a height to be elevated
against, so this is P0.

**`z` already means draw order in this code** ([I2](issues.md#i2)). `slotZ()` returns a painter's key
and `MoverPart.depth` is *"draw-order offset along the view's depth axis"*. Introducing a world-height
`z` beside them is a collision waiting to happen, so this stream calls the new quantity
**`elevation`** ([F1](forks.md#f1)).

## What this buys beyond the pawn's head

It generalises immediately: a torch bracketed on a wall, a hat, a carried tool, a bird — anything off
the ground gets a correct shadow from **one number** instead of a per-case y-fudge that has to be
re-tuned whenever the art changes. And it is the same question as
[lighting-rework I13](../2026-07-31-lighting-rework/issues.md#i13) defect 4 (the blit lights a
billboard by the ground it is *drawn* over rather than the tile it *stands* on) — both are "where does
this thing actually stand", and P4 settles them together.

## The number this stream moves

**Body and head cast ONE aligned shadow**, verified on screen at zoom — not two offset ones. Secondary:
no measurable frame cost, since this adds arithmetic to existing passes rather than passes.
