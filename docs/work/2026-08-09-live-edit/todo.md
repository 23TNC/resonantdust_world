# Plan — live-edit

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions in
[`forks.md`](forks.md) (F#), the anticipated-issue inventory in [`issues.md`](issues.md) (I#)._

**How acceptance is measured.** `npm run build` green + **no new** type errors against the
7-error baseline (`2026-08-09-panel-grid` I11), plus measured browser evidence via
`:5174/?user=Claude&focus=100,50&zoom=1&cb=area1`. Rust changes additionally need the wasm rebuilt
and the client restarted.

**Ordering is deliberate**: the cheap, certain tabs land first so the panel is real and useful
before the expensive parts start, and the preview spike runs early because its answer changes a
week of work.

## P0 — the paper

- [x] Design doc `docs/components/client/webgl/design/live-edit.md`: the panel's shape, the ONE
      snapshot rule (F5), read-only stance (F6), and the four tabs' data sources. Acceptance:
      docs-check green; work index row.

## P1 — the panel exists

- [x] `/edit` registered via `ChatPanel.registerCommand`, opening the panel bound to the current
      selection (F4). Acceptance: `/edit` with a pawn selected opens it; with nothing selected it
      opens with empty tabs rather than refusing.
- [x] `LiveEditPanel` with a preview region above and a hand-rolled tab strip below, styled like
      the built-in tabs (I8). Acceptance: switching tabs swaps content and leaves the preview
      mounted.
- [x] The panel follows the selection model and re-binds (F4). Acceptance: selecting another pawn
      re-fills every tab; selecting a tile empties them.
- [x] Authored `defaultCell` + a raised `minCols`/`minRows` reflecting what it needs (I7).
      Acceptance: cleared storage opens it on-grid at a usable size.

## P2 — the cheap tabs

- [x] Emotions tab: labels + live values from `pawn_emotion`, coloured by `emotion_color`.
      Acceptance: values change as a pawn's conditions change; capture.
- [x] Conditions tab: pie squares in a grid with name-on-hover, reusing the existing card
      rendering. Acceptance: matches the conditions panel's squares for the same pawn.
- [x] ONE eval snapshot per refresh, sliced to the tabs; only the ACTIVE tab renders (F5).
      Acceptance: eval call count per refresh is 1 with all four tabs present (counter logged).

## P3 — trait colour and the traits tab

- [x] `color` on the trait def: loader field, `#rrggbb` parse, neutral fallback when unauthored
      (F2). Acceptance: unit/load green; an unauthored trait loads.
- [x] `trait_color` + a `pawn_traits` accessor returning `(reference, label, colour)`, mirroring
      `pawn_conditions` (F2/I2). Acceptance: a drill pawn's traits come back with labels.
- [x] Author a colour on every trait in `content/`; golden fixture re-blessed with ONLY the new
      column moving (I5). Acceptance: registry seeds with zero divergence.
- [x] Traits tab: colour squares in a grid, name on hover. Acceptance: a pawn's traits render with
      authored colours; capture.

## P4 — need colour, bounds and the live rate

- [x] `color` on the need def, same ritual as the trait colour (F2). Acceptance: load green.
- [x] One accessor returning a need's live `(value, min, max, rate)` from a single evaluation —
      effective clamp, not authored bounds (F3/I3). Acceptance: a pawn under a rate-modifying
      condition reports a different rate from one without.
- [x] Fix the rate's sign convention in the accessor so "down is red" holds for every need
      including inverted domains (I4). Acceptance: thirst and inventory both read correctly.
- [x] Author a colour on every need; golden re-blessed (I5). Acceptance: registry seeds clean.
- [x] Needs tab: label + bar filled from value against the EFFECTIVE clamp + signed rate, red
      negative / green positive (F3). Acceptance: drinking visibly moves the bar and flips the
      rate's sign; capture.

## P5 — the preview

- [x] SPIKE: instantiate a second `Viewport`, measure context creation, texture residency and
      frame cost, and check repeated open/close for context leaks (F1/I1). Acceptance: numbers in
      completed.md and a recorded decision between A and B.
- [ ] BLOCKED — build the preview. The spike's verdict was necessary but not sufficient: a second
      `Viewport` has no CONTENT ([I10](issues.md#i10)). Three costed options + a recommendation in
      `blockers.md`; needs the user's call on the renderer's shape.
- [ ] Zoom and pan on the preview. Acceptance: both work without disturbing the world viewport's
      own camera.
- [ ] Non-pawn selections degrade gracefully (I6). Acceptance: selecting a tile or thing leaves
      the preview coherent, never half-initialised.

## P6 — the truth

- [ ] Docs + memory truth pass; the full exit on camera — `/edit`, all four tabs against a live
      pawn, the rate moving as it drinks; **the user's eyes close the stream**. Acceptance:
      captures in completed.md; docs-check green.
