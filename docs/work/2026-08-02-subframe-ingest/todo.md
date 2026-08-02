# Plan — subframe at ingest

_The plan for the life of the stream. Items never move; `[x]` IS the move. Context in
[`README.md`](README.md)._

**Acceptance for the whole stream.**

- **Zero fringe rows, measured.** The receiver map's covered texels must match the drawn silhouette's
  texels with no offset band — the `100,48` bush is the fixture that found the bug and is the one
  that must come back clean.
- **The plan line lands on the authored anchor.** A prim's stored ground row equals its `.rd` anchor,
  which closes [lighting-visual I1](../2026-07-31-lighting-visual/issues.md).
- **One rect, four maps.** Albedo, normal, surface and layers are cropped by the SAME draw — the
  property `packCoPack` already has and must keep.
- **The frame stays square pow2** ([I3](issues.md#i3)). A non-square quadrant breaks `ppu` silently.
- **Never re-derive geometry from pixels at runtime** ([I5](issues.md#i5)). The bbox proposes a
  number to an author; it must not feed placement again.

**Fixture:** `?user=Claude&focus=100,48&zoom=4`, one torch seeded at `(100,50)`, then `__zprobe(100,48)`
for coverage and `__elev(i)` for the anchor.

## P0 — Measure the baseline before authoring anything

- [x] Record the current receiver-vs-drawn fringe for the `100,48` bush. Acceptance: the covered-texel count and the topmost covered row are written down, so "no fringe" later is a comparison and not an impression.
- [x] Measure opaque-fraction-of-frame across the corpus ([I1](issues.md#i1)). Acceptance: a per-stem table plus the average, so the resolution claim is sized before it is made.
- [x] Record where each prim's plan line sits today versus its art's feet. Acceptance: the letterbox offset is quantified per stem, which is the number [lighting-visual I1](../2026-07-31-lighting-visual/issues.md) closes on.
- [x] Confirm `packCoPack` applies one draw to all four sources. Acceptance: stated with the line quoted, because the whole design rests on that rect being shared and nothing today enforces it.

## P1 — Author subframe + anchor in the DSL, per direction ([F1](forks.md#f1), [F4](forks.md#f4))

- [ ] Add `&thing.subframe.x/y/w/h` as FRACTIONS to `shared/dsl` loader + `VisualParts`. Acceptance: a `.rd` authoring them round-trips through the loader's own test corpus, defaults being the full frame `(0,0,1,1)`.
- [ ] Add `&thing.anchor_point.x/y` as fractions of the SUBFRAME. Acceptance: it round-trips, and its default reproduces today's bottom-centre so an unauthored stem does not move.
- [ ] Make both PER DIRECTION, keyed the way the resolver already keys stems. Acceptance: e/s/n each carry their own values through the loader, since each is a genuinely different texture.
- [ ] Derive west from east by mirroring, never authoring it ([F4](forks.md#f4)). Acceptance: a west stem's subframe is east's mirrored about the frame centre, with the rule stated where someone would be tempted to author it.
- [ ] Author the real values for the corpus's stems. Acceptance: every mastered stem has a subframe matching its art, taken from the measured bbox ([I5](issues.md#i5)), not guessed.
- [ ] Record the new variables in `VARIABLES.md`. Acceptance: the layout doc carries them, since it outranks the code on layouts and a DSL variable absent from it is invisible.

## P2 — Crop at ingest: one rect, four maps ([F2](forks.md#f2), [F5](forks.md#f5))

- [ ] Carry the subframe + anchor to the resolver through `WorldBridge`, as `setSpriteScale` already is. Acceptance: a change evicts the stem's packed frames, matching the existing eviction discipline.
- [ ] Compute ONE `AtlasDraw` from the subframe and apply it to all four sources in `packCoPack`. Acceptance: the four maps land quadrant-aligned from a single rect; a deliberately offset test subframe moves all four together.
- [ ] Scale-to-fit preserving aspect, positioned by the anchor ([F2](forks.md#f2)). Acceptance: a non-square subframe is not distorted, and the residual margin sits on one axis only.
- [ ] Compose with `sprite_scale` rather than double-placing ([F5](forks.md#f5)). Acceptance: `sprite_scale` multiplies the target rect and no longer performs its own pivot re-centre; a stem with both authored is placed once.
- [ ] Apply the subframe per CELL, not only per stem ([F6](forks.md#f6)). Acceptance: a linked stem's autotile cells each resolve their own subframe, using the `(stem, cell)` keying the resolver already has.
- [ ] Delete `internal_padding` and express it as a subframe ([F6](forks.md#f6)). Acceptance: the DSL variable, `setLinkedPad` and the resolver's pad path are gone, and linked tiles inset identically to before under an authored subframe.
- [ ] Re-examine the `entry.grid` exclusion that blocks `sprite_scale` ([F5](forks.md#f5), [F6](forks.md#f6)). Acceptance: a stated answer now that placement and world extent are split — grids stop being a special case, or the reason they stay one is written down.

## P3 — The anchor lane, and the end of the runtime bbox ([F3](forks.md#f3), [I5](issues.md#i5))

- [ ] Put the anchor in the def's freed GREEN word as `u16 x | u16 y` in sixteenths. Acceptance: the layout note at the top of `records.ts` describes the lane it now holds, replacing the "free" note left for it.
- [ ] Read the anchor in `recordSync.groundRowOf` instead of `p.y + p.height`. Acceptance: the stored ground row equals the authored anchor, and [lighting-visual I1](../2026-07-31-lighting-visual/issues.md) is closed with evidence.
- [ ] Delete `opaqueBBox` from the runtime geometry path. Acceptance: no placement or sampling code consults a pixel-derived bbox; `grep opaqueBBox` returns only the dev-time tool and its callers.
- [ ] Keep the measurement as an AUTHORING tool ([I5](issues.md#i5)). Acceptance: a probe or `bin/art` output prints a stem's measured bbox in the `.rd`'s own units, so authoring a subframe is not guesswork.

## P4 — Verify, and guard against silent drift ([I6](issues.md#i6))

- [ ] Re-measure the bush's fringe against P0. Acceptance: zero offset rows between the receiver map's coverage and the drawn silhouette — the halo is gone at its source, not masked.
- [ ] Re-measure the plan line against P0. Acceptance: every prim's ground row sits on its art's feet, quantified per stem the same way P0 quantified the error.
- [ ] Re-measure opaque-fraction-of-frame against P0's table ([I1](issues.md#i1)). Acceptance: the resolution change is stated as a number, including "no change" if the corpus was already tight.
- [ ] Warn when an authored subframe disagrees with the measured bbox ([I6](issues.md#i6)). Acceptance: a regenerated master that moves its art inside the frame produces a console warning rather than a silent lighting bug.
- [ ] Confirm the four maps stayed registered. Acceptance: a normal-mapped stem lights consistently with its own silhouette, which is the failure the shared rect exists to prevent.
- [ ] Record the ingest contract in `design/de-lighting.md`. Acceptance: it states that the atlas holds CROPPED art addressed by an authored subframe + anchor, so the next generator or resolver change collides with it.
