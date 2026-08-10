# Plan — object-preview

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions in
[`forks.md`](forks.md) (F#), the anticipated-issue inventory in [`issues.md`](issues.md) (I#)._

**How acceptance is measured.** `npm run build` green and `npx tsc --noEmit` at **zero** errors
(the client reached zero on 2026-08-09 and should stay there), plus measured browser evidence via
`:5174/?user=Claude&focus=100,50&zoom=1&cb=area1`. Rust changes need the wasm rebuilt
(`bin/rd build shared`) and the client reloaded.

## P0 — the paper

- [ ] Design doc section (extend `design/live-edit.md`'s preview paragraph, or a sibling):
      2D canvas, no GL, resolver-as-source-of-truth, parts composited (F1–F3). Acceptance:
      docs-check green; work index row.

## P1 — the read-only seam

- [ ] A narrow read-only accessor surface on `TextureResolver`: packed hash + size for a stem,
      its sub-rect, bbox and sprite scale (F2/I1). Acceptance: data only — no method on it can
      fetch, pack or upload.
- [ ] A bitmap cache keyed `(stem, size, hash)` that outlives panel opens, fetching the ALBEDO
      map only (I2/I5). Acceptance: selecting the same kind twice fetches once (counter logged).

## P2 — drawing one object

- [ ] An `ObjectPreview` owning a 2D canvas, drawing one part from a cached bitmap with
      `imageSmoothingEnabled = false`, re-applied after resize (F1/I4). Acceptance: a pawn's body
      renders crisp at 4× zoom.
- [ ] Composite ALL parts in `zIndex` order with their offsets and scales (F3/I7). Acceptance: a
      two-part pawn shows body AND head, correctly stacked.
- [ ] Honour each part's tint and `flipX` (I6). Acceptance: an east-facing tinted pawn matches
      its world appearance; capture both.
- [ ] Geo fallback — the kind's colour + glyph — when no bitmap has resolved (F6/I3).
      Acceptance: a placeholder-art thing previews as a coloured box with its letter, never blank.

## P3 — the camera

- [ ] Fit-to-object on selection: the composed bbox scaled to the region with a margin (F4/I7).
      Acceptance: a tile, a bunny and a tree each open framed, without manual zoom.
- [ ] Wheel zoom and drag pan as a canvas transform (F4). Acceptance: both work; the world
      viewport's own camera is provably untouched.
- [ ] Stop re-fitting once the user has adjusted, with a control to resume (F4). Acceptance: after
      panning, a selection change leaves the view alone; the control re-fits.

## P4 — into the panel

- [ ] Mount `ObjectPreview` in `LiveEditPanel`'s preview region, fed from the snapshot's selection
      (live-edit F5's one-snapshot rule still holds). Acceptance: `/edit` shows the selected object.
- [ ] Non-pawn selections preview through the same path (F6). Acceptance: selecting a tile and a
      thing each render something honest.
- [ ] DELETE the placeholder note and `PreviewViewport.ts` (I8). Acceptance: no second, GL-based
      preview implementation remains in the tree.

## P5 — the truth

- [ ] Prove no WebGL context is created by the preview and the world viewport is unaffected
      (F1/live-edit I1). Acceptance: context count unchanged across repeated `/edit` open/close;
      world context alive.
- [ ] Docs + memory truth pass; the full exit on camera; **the user's eyes close the stream**.
      Acceptance: captures in completed.md; docs-check green.
