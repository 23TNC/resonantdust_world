# Pawn part placement — plan

Items are checkboxes; tick in place (`[x]`), never move them. One action + acceptance each.
Ordering in [`README.md`](README.md): **verify what already works**, then the cheap DSL tune, then
the two real gaps (facing z-order, albedo/lighting unification), then re-check the normal.

## P0 — Verify before changing anything

- [ ] Capture a zoom-1 screenshot of a human pawn in all four facings as the before-image. Acceptance: four crops saved, so every later change has something to diff against.
- [ ] Confirm the head slot is DSL-placed today by editing `head.offset.y` and reloading. Acceptance: the head visibly moves, proving requirement 6 is already met and no pixel constant overrides it.
- [ ] Read back the wasm `moverParts` row for a live human. Acceptance: `scale`, `offsetX/Y`, `part` and `span` print the corpus values, or the mismatch is recorded in `issues.md`.
- [ ] Compare the pawn's normal and albedo atlas frame sides in the console. Acceptance: a number for each; if equal, [I2](issues.md#i2) is refuted and P4 shrinks to a doc fix.
- [ ] Measure how far a head fragment's lightmap sample sits from its own footprint row. Acceptance: a tile count recorded in `issues.md`, confirming or refuting the ~1-tile estimate in [I1](issues.md#i1).

## P1 — The DSL tune (body 0.8, head 0.5)

- [ ] Set `body.size` to `0.8` for both `human_female` and `human_male`. Acceptance: the pawn draws 0.8 tiles tall and the head is 0.5 by the existing 0.625 relative scale — no `head.scale` edit.
- [ ] Re-seat `head.offset.y` for the new body height, starting from half a body height north. Acceptance: the head sits ON the shoulders, not inside the torso, at zoom 1 in the east facing.
- [ ] Replace the stale "first-guess placement" comment in `pawns.rd` with the measured value. Acceptance: the comment names the body height it was tuned against, so the next size change knows to re-tune.
- [ ] Check the head still seats correctly in all four facings. Acceptance: four crops; any facing that needs its own offset is recorded as a finding rather than special-cased.

## P2 — Facing-dependent z-order

- [ ] Add a per-slot depth field to the DSL that flips with facing. Acceptance: [F1](forks.md#f1) names the field, its sign convention, and why it is per-slot rather than "the head".
- [ ] Carry that field through the wasm `moverParts` row into `MoverPart`. Acceptance: the console read-back from P0 shows the new field carrying its authored value.
- [ ] Give each slot its own `zIndex` instead of reusing slot 0's. Acceptance: `MoverLayer` no longer computes one `zIndex` for every spec; the head and body can differ.
- [ ] Order the head over the body for e/w/s and under it for n. Acceptance: four crops — the head occludes the body in three facings and is occluded in north.
- [ ] Confirm the change respects the blit's warm-over-cold row compare. Acceptance: a pawn walking north past another pawn still sorts correctly, with no flicker at the row boundary.

## P3 — One positioning system for albedo and lighting

- [ ] Establish whether the hot lightmap's mover deposit is positioned at the drawn coverage or the footprint. Acceptance: the answer is stated with a file:line, because it decides whether the blit alone can move.
- [ ] Make a billboard fragment recover its footprint row from `zdepth_world.b`. Acceptance: a debug overlay shows the recovered row matching the pawn's actual tile for every fragment of the sprite.
- [ ] Sample the lightmap at the footprint row for billboard fragments, leaving ground fragments unchanged. Acceptance: ground lighting is byte-identical; the pawn's lighting no longer shifts as it moves north/south.
- [ ] Verify the vertical gradient loss is acceptable at zoom 1. Acceptance: a before/after crop under a low light; if the flat result reads wrong, record it and stop rather than tuning blind.
- [ ] Re-run the lighting-cost measurement. Acceptance: the lighting pass ms is within noise of the P0 baseline — the fix is a sample-address change, not extra work.

## P4 — Normal scaling

- [ ] Act on P0's frame-side comparison. Acceptance: if the sides differ, the stage that diverges is named; if they match, this phase closes as refuted with a note in `completed.md`.
- [ ] Make the normal follow the same DSL scale as the albedo wherever P0 found the divergence. Acceptance: both atlas frames report the same side for a pawn at `body.size 0.8`.
- [ ] Confirm the shading direction still reads correctly after any scale change. Acceptance: a lit crop shows the pawn lit from the light's side, with `worldNormal`'s pitch untouched.

## P5 — Close out

- [ ] Re-run the four-facing capture and diff against P0's before-image. Acceptance: `completed.md` states what changed per facing, and any remaining defect is recorded rather than implied fixed.
- [ ] Record whether `SPEC_APPLY_EPS` still reads as judder at the new body size. Acceptance: a yes/no in `issues.md`, so the movement question is either closed or handed on deliberately.
- [ ] Run `bin/rd docs-check` and close the stream. Acceptance: tree green, `completed.md` carries the before/after crops and the measured values.
