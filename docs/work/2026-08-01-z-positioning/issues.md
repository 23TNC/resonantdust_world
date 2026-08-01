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
