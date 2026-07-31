# Forks — pawn part placement

_A choice I resolved, with what was rejected and why. A fork is mine; a [blocker](blockers.md) is
the user's._

## F1 — How facing-dependent z-order is expressed {#f1}
_2026-07-30 · resolved at plan time · **a per-SLOT depth offset that flips with facing**_

**Chosen.** Author a per-slot depth offset in the DSL (alongside `offset.x`/`offset.y`), and flip
its sign — or not — from the facing. "Head over body for e/w/s, under for n" becomes a slot whose
depth is negative when the pawn faces away.

**Why not special-case the head.** `human-pawns` records that a slot's content rides the pawn's
payload (`PART(slot, def)`), so an armour or backpack def can replace a slot later. A backpack is
the same rule with the opposite sign — behind when facing SOUTH. Hard-coding "slot 1 is the head
and heads go behind when facing north" makes that a second code change; a per-slot depth makes it
an authoring change.

**Why not reuse `offset.y`.** Nudging the head further north to fake ordering would move it in
WORLD space, changing where it seats on the body and where it samples light. Depth and position
are different axes and must stay separable.

**Rejected — derive it from the art.** A head drawn facing away is still a head; nothing in the
pixels says "sort me behind".

## F2 — Where the albedo/lighting unification happens {#f2}
_2026-07-30 · resolved at plan time · **recover the footprint in the deferred pass**_

**Chosen.** Have the blit recover a billboard fragment's footprint row from `zdepth_world.b` and
sample the lightmap there, leaving ground fragments sampling where they are drawn.

**Why this side.** The information already exists: `mrtBakeShader` attachment 3 writes the prim's
z-row into B with a high bit distinguishing billboard from ground, and `albedoBlitShader` ALREADY
reads it (`texture(uDepth, vUV).b * 255.0`) for the warm-over-cold occlusion compare. So no new
G-buffer channel, no new bake work — it is a sample-address change in a shader that already has
the address.

**Rejected — bake the light into the prim.** Would make lighting a per-prim bake instead of a
world-space field, losing the whole point of the toroidal lightmap.

**Rejected — move the lightmap deposit to the drawn position.** The lightmap is a GROUND field
shared by terrain and shadows; distorting it to suit billboards would break everything that reads
it correctly today.

**Known consequence, deliberately accepted:** sampling every pixel of a sprite at its footprint
gives the whole sprite one tile's irradiance, so the vertical gradient up the sprite disappears.
That is arguably correct — position from the footprint, DIRECTION from the already-pitched normal
(`worldNormal(n, phi)`) — and a 2D ground lightmap cannot represent height anyway. [P3](todo.md)
checks it by eye before accepting.

## F3 — Which field sizes a part {#f3}
_2026-07-30 · **RE-RESOLVED at P0.3** — the original was built on a field that does nothing
([I4](issues.md#i4)) · **slot 0 gets its own `scale`, like every other slot**_

**Chosen.** Apply `MoverPart.scale` to slot 0's carrier box, the same way it already applies to
slot 1. Then `0.8 &body.scale set` draws the body at `128 × 0.8 = 102.4` px, and the head follows
at `0.625 × 0.8 = 0.5` tiles — the user's two numbers, from one authored value each.

**Why the original was wrong.** It said to move `body.size` 1.5 → 0.8. Measured live, the body
draws at exactly 128 px = one tile, because `placeThing` sizes the box off **`span`**, not `size`
— `body.size 1.5` is inert. Editing an ignored field would have changed nothing, and `span` cannot
express 0.8 at all (it is pow2 TILES).

**Why not make `size` work again.** `loader.rs` marks `size` *"SUPERSEDED by `span` +
`sprite_scale` (def-frame-anchors P5) — kept while legacy consumers migrate."* Reviving a field the
design is retiring would move against it.

**Why not `sprite_scale`.** It is applied PRE-ATLAS at ingest, so it changes the texels baked into
the frame, not the world size the frame is drawn at. Wrong axis.

**Why slot 0's `scale` is the right home.** It already exists, is already authored per slot, and
already works — the head's `w 80 = 128 × 0.625` proves the path. Slot 0 is simply the one slot
whose `scale` is currently ignored, because `size0` is taken straight from `placeThing`. Making it
uniform removes a special case rather than adding one.

**Knock-on for the head offset:** `MoverLayer` derives `tilePx = size0 / slots[0].size`, which
divides by the very `size` that does not size the box — the likely reason `head.offset.y -1.15`
resolves to a 10 px separation instead of ~147 px. That divisor has to be re-derived from whatever
sizes the box once this lands.

## F3-original — superseded, kept for the record
_2026-07-30 · resolved at plan time · **only `body.size` moves**_

**Chosen.** Set `body.size` from `1.5` to `0.8` and leave `head.scale` at `0.625`.

**Why.** `MoverPart.scale` is documented as multiplying *slot 0's drawn size*, i.e. it is RELATIVE.
`0.625 × 0.8 = 0.5` exactly — the user's two numbers are already consistent with the authored
relative scale. Setting `head.scale` to `0.5` as well would apply it twice and land the head at
`0.4`.

**Consequence for [P1](todo.md):** `head.offset.y` must be re-tuned regardless, because `-1.15`
was fitted against a body drawn at `1.5` tiles. The offset is in TILES from slot 0's anchor, so it
does not rescale with the body automatically.
