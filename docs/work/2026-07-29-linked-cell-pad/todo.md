# Plan — per-cell padding on linked textures, authored in the DSL

_The plan for the life of the stream. Items never move; `[x]` IS the move. Context in
[`README.md`](README.md)._

**Acceptance for the whole stream.** Every phase must hold these:

- **A def that authors no pad renders bit-identically to today.** The field defaults to 0 and the
  manifest path stays live, so nothing that does not opt in can change.
- **One authored number is correct at every lod.** The pad is a fraction of a cell, not a pixel count
  against a particular sheet size — verified at two lods, not one.
- **No `slot` naming.** The textile slot grid owns that word ([F1](forks.md#f1)).

**Fixture:** area1 at zoom 1 (`?user=Claude&focus=100,50&zoom=1`), the `wall_smooth` tile — the only
mastered linked kind (`biome-tile/default/smooth/wall`, grid 4×4).

## P0 — Measure the cell geometry before changing anything

- [ ] Read back the wall's live manifest entry — `maxSize`, `grid`, `pad` — and the co-packed frame side actually loaded at zoom 1. Acceptance: the native cell px is a recorded number, since every unit conversion in P2 depends on it.
- [ ] Compute and record the cell px at each lod the fixture reaches. Acceptance: a table lod → sheet px → cell px, so the LOD-invariance claim in P2 has something to check against.
- [ ] Capture a zoom-1 crop of a wall run as the before-image, at a spot where two different cells meet. Acceptance: the image is saved and the cell boundary is identifiable in it.
- [ ] Confirm `cellFrame`'s output is stretched, not resampled, by checking the consumer's sampling of the returned `TexFrame`. Acceptance: states plainly whether the user's "scale up to 128" step needs any code at all ([I1](issues.md)).

## P1 — Author the field in the DSL (no consumer yet)

- [ ] Add `texture_pad` to `VisualParts` in `shared/dsl/src/loader.rs`, read via `read_f("prims.0.texture_pad", 0.0)`. Acceptance: `cargo build` green; a def that sets it round-trips the value, one that does not reads 0.
- [ ] Author `0 &tile.texture_pad set` on `wall_smooth` with a comment naming the unit (px per cell edge at the art's native cell size). Acceptance: explicit zero, so the field's presence is not itself a behaviour change.
- [ ] Rebuild the wasm and confirm the value reaches the client through `visual_for_def`. Acceptance: read the field back in the console for the wall def; it is 0.
- [ ] Verify nothing rendered changed. Acceptance: the P0 before-image and a fresh capture are indistinguishable.

## P2 — Consume it in the resolver

- [ ] Convert the authored px to `cellFrame`'s normalized units at the call site: `pu = (padPx / nativeCellPx) / cols`, same for `pv` with `rows`. Acceptance: hand-check one value against the P0 table.
- [ ] Thread the def's pad into `resolve()` so `cellFrame` prefers it, falling back to `entry.pad` when the def authors none ([F3](forks.md#f3)). Acceptance: a def with no pad still gets the manifest's.
- [ ] Prove the swap is inert: author the wall's CURRENT manifest pad as its DSL value. Acceptance: renders indistinguishably from P0 — same number, different source.
- [ ] Confirm the LOD-invariance claim by forcing two different lods with one authored value. Acceptance: the trimmed region is the same fraction of the cell at both; no drift.

## P3 — Author the real value and decide the rescale question by eye

- [ ] Set `wall_smooth`'s pad to the art's actual guard ring and capture the same crop as P0. Acceptance: side-by-side shows no bleed from neighbouring cells at the boundary.
- [ ] Inspect the 126→128 stretch for a visible artifact — duplicated rows/columns in the tiling material ([F5](forks.md#f5)). Acceptance: a zoomed crop, and a stated verdict: acceptable, or F5 needs its fallback.
- [ ] If the artifact reads badly, apply F5's chosen fallback and re-capture. Acceptance: either the artifact is gone, or the deviation is recorded with why it was accepted.
- [ ] Check a non-linked sprite (the conifer) is untouched. Acceptance: identical to before — `cellFrame` only runs for defs with a grid.

## P4 — Docs

- [ ] Add `texture_pad` to the asset-path / texture vocabulary docs with its unit and its conversion. Acceptance: a reader can author a correct value without reading the resolver.
- [ ] Record in `VARIABLES.md` that linked cells are the one map family exempt from whole-px-per-unit, with the reason. Acceptance: the exemption is stated where the invariant is stated, not only here.
- [ ] Run `bin/rd docs-check` and close the stream. Acceptance: tree green, `completed.md` records the before/after crops and the F5 verdict.
