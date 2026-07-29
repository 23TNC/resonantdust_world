# Todo — marigold-linked-normals

_The oracle is STAGED: P0's numbers gate everything after; P4 is the user's eyes.
Design + fork rationale: [`README`](README.md)._

---

## P0 — the consistency oracle (measure before touching generation)

- [x] `marigold/atlas_check.py`: per-cell FLAT-frame error — cluster each cell's normals,
      report the dominant (flat-top) cluster's mean angular deviation from +Z, per cell +
      worst. Acceptance: runs on the smooth wall atlas; prints a per-cell table.
- [x] Same-piece deviation in `atlas_check.py`: using the D1 cell semantics (N/E/S/W bits
      → which run/corner pieces a cell contains), report cross-cell angular deviation of
      equivalent piece regions. Acceptance: numbers for the smooth wall; worst pair named.
- [x] Seam continuity in `atlas_check.py`: for every in-world-valid edge pair (an
      E-connected cell's east strip vs a W-connected cell's west strip, same for N/S),
      report mean/max edge-strip normal difference. Acceptance: a seam table prints; the
      current atlas's numbers are recorded in `completed.md` as the baseline.

## P1 — inference hygiene (per-cell, deterministic)

- [x] `marigold/atlas_normals.py`: slice the diffuse atlas into cells, pad each crop with
      REPLICATED edges (never the atlas neighbour), run the normals pipeline per cell with
      pinned seed + ensemble, reassemble. Acceptance: output atlas same size/encoding;
      `atlas_check` runs on it.
- [x] Wire `bin/art normal --marigold` to route grid kinds (`_is_grid_cat`) through
      `atlas_normals.py`; non-grid kinds unchanged; engine stamp still `marigold`.
      Acceptance: `bin/art normal --marigold <wall-kind>` produces the per-cell atlas;
      a non-grid kind still takes the old path.
- [x] Record P1 `atlas_check` numbers vs the P0 baseline. Acceptance: cross-cell bleed
      (border-gradient artifacts at cell boundaries) gone; numbers in `completed.md`.

## P2 — global frame alignment

- [x] Per-cell flat-frame correction in `atlas_normals.py`: dominant-cluster mean → the
      rotation taking it to +Z, applied to every normal in the cell (vector rotation, then
      re-encode). Acceptance: `atlas_check` flat-frame error ≈ 0 for every cell.
- [x] Bevel-gain alignment: per cell, align X/Y slope distributions to the reference cell
      (matched over piece regions only, not flats). Acceptance: same-piece deviation drops
      vs P1; numbers recorded.

## P3 — symmetry + seam enforcement

- [x] Equivalent-piece averaging: average each piece across all cells containing it, under
      the piece's mirror transform (flip pixels AND negate the mirrored normal component);
      write the average back to every instance. Acceptance: same-piece deviation ≈ 0.
- [x] Seam blending: force in-world-valid edge pairs to exact agreement (average the strip
      pair, feather inward ≤2 px). Acceptance: `atlas_check` seam table reads ≈ 0 on all
      valid pairs; no visible ridge in the atlas image.

## P4 — regenerate + the in-game drill

- [x] Regenerate the smooth wall normals through the new path + re-export/serve (the edge
      re-derives on mtime). Acceptance: the served atlas hash changes; `atlas_check` final
      numbers recorded in `completed.md`.
- [x] In-game drill: the torch-side wall compound at zoom 1 + zoom 2 — one continuous wall
      run shades as ONE surface (no per-tile jumps), corners/bevels read consistently.
      Acceptance: captures in `completed.md`; the user's eyes are the final oracle.
