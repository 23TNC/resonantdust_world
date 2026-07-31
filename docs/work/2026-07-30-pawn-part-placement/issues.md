# Issues — pawn part placement

_Problems hit, and what the evidence actually showed. Findings recorded at plan time are marked as
such — they were read out of the code, not measured under this stream._

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
_2026-07-30 · read at plan time from `SquareCache.ts` + `mrtBakeShader.ts` · **needs measurement**_

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
