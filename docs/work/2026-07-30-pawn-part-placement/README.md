# Pawn parts: seat the head, order it by facing, and stop albedo/lighting disagreeing — 2026-07-30

_Components: [`client/webgl`](../../components/client/webgl/) (`MoverLayer.ts`,
`thingPlacement.ts`, `albedoBlitShader.ts`) · [`shared/dsl`](../../components/shared/) ·
[`dev/art`](../../components/dev/). Plan in [`todo.md`](todo.md); decisions in
[`forks.md`](forks.md); findings in [`issues.md`](issues.md). Follows
[`2026-07-30-human-pawns`](../2026-07-30-human-pawns/README.md) (done), which BUILT the part-slot
skeleton this stream tunes and extends._

## What the user asked for

1. Human pawns display correctly.
2. **Z-order by facing** — head over body for e/w/s; head **under** body for n.
3. **Head seated north by about half a body height**, so it sits on the body, not inside it.
4. **One positioning system for albedo and lighting**, so they cannot drift apart.
5. **Normal scales with albedo**, driven by the DSL.
6. Head placed from **DSL variables, not hardcoded**.
7. **Body scale 0.8, head scale 0.5.**

## Most of the mechanism already exists — this is tuning plus two real gaps

The done [`human-pawns`](../2026-07-30-human-pawns/README.md) stream built the slot skeleton, and
`content/visual/pawns.rd` already authors it. `MoverPart` carries per-slot `part`, `scale`,
`offsetX/offsetY`, `span`, `anchor*`, `spriteAnchor*`, and its own doc says *"`scale` multiplies
slot 0's drawn size; `offset*` places the slot in tiles relative to slot 0's anchor."* So
requirement 6 is **already satisfied** — the head is DSL-placed today, at
`0.625 &head.scale set` / `-1.15 &head.offset.y set`.

**The two scale numbers are already consistent.** `head.scale` is *relative to the body's drawn
size*, and `0.625 × 0.8 = 0.5` exactly. So requirement 7 is one edit — `body.size` from `1.5` to
`0.8` — and the head's 0.5 falls out. Changing `head.scale` as well would double-apply it.

That leaves the genuinely new work:

- **Facing-dependent z-order (2).** Every slot currently shares one `zIndex`:
  `const zIndex = PAWN_Z_BASE + box.zRow` is computed once from slot 0 and reused. There is no
  per-slot ordering to express "head behind when facing north".
- **Albedo/lighting unification (4).** `albedoBlitShader` sets `vWorld = aPosition` and reads
  `lightTexel(vWorld)`, so every fragment samples the lightmap at the ground point it is **drawn
  over**. A standing billboard's pixels are drawn up-screen of its footprint, so it is lit by
  ground it does not stand on — up to a full tile (128 world px at `SQUARE` 128), and it scales
  with `size`. This is the misalignment; see [I1](issues.md#i1).

## Design stance

- **Tune the skeleton, do not rebuild it.** The slot system, the DSL fields and the wasm
  `moverParts` row all exist and work. Items here change values and add ordering — anything that
  wants a new mechanism is a signal to re-read `human-pawns` first.
- **The corpus authors, the client obeys.** `head.offset.y` is the placement; no pixel constant
  in `MoverLayer` may encode it. Same rule as [art-128-tiles F1](../2026-07-28-art-128-tiles/forks.md#f1).
- **Unify by making the deferred pass recover the footprint, not by moving the bake.** The row is
  already in the G-buffer: attachment 3 is `zdepth_world` with the prim's z-row in B and a high
  bit marking billboard-vs-ground, and the blit ALREADY reads it for warm-over-cold occlusion.
  So the sample position is recoverable without a new channel ([F2](forks.md#f2)).
- **Direction is already right; only position is wrong.** `worldNormal(n, phi)` pitches a standing
  billboard's normal into the world frame by `90° − tilt`. Do not touch it while fixing position.
- **Verify the normal-scale claim before acting on it.** `SquareCache` bakes albedo/normal/surface/
  zdepth from ONE prim quad through a merged MRT pass, so a geometric scale cannot reach albedo
  alone at bake time. If the normal really is scaling differently, it happens earlier — in the art
  pipeline or at atlas ingest — and [P0](todo.md) finds out which before [P4](todo.md) changes
  anything.

## Future intent this plan must not trim

- **Parts are a skeleton for equipment, not just heads.** `human-pawns` records that a slot's
  content rides the pawn's payload (`PART(slot, def)`), so an armour def can later replace a slot.
  Per-slot ordering and offsets must stay expressed per SLOT, never special-cased to "the head".
- **The variant grouping is character-gen input.** `fit = 0,1,2 · fat = 3,4,5 · average = 6,7,8`,
  16 heads per sex, any head fitting any body of the same sex. Recorded so it survives.
- **The z-order rule generalises.** "Head behind when facing north" is really "a slot's depth
  offset flips with facing" — author it so a backpack (behind when facing south) needs no new code.
