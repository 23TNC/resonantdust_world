# Todo — analytic-wall-normals

_atlas_check gates every phase; the user's eyes ratify P2 and P3. Design + forks:
[`README`](README.md)._

---

## P0 — the profile, fitted to the drawn art

- [x] Measure the smooth wall's cross-section from the art: extract a run cell's profile
      bands (outline px, front-face extent, bevel band, top-face) from the diffuse along
      the run's cross-section; write them to `geometry.json` in the kind dir. Acceptance:
      the sidecar exists with measured px values + a plot/strip capture in the folder.
- [x] A REGISTER check (in `atlas_geometry.py --check`): the profile's band boundaries vs
      the art's luminance-gradient bands on a run cell — reported as px offset per band.
      Acceptance: runs on the smooth wall; offsets ≤ 1 px after the P0 fit.

## P1 — the analytic generator

- [x] `marigold/atlas_geometry.py`: per-cell wall SHAPE from the D1 bits (arms + hub,
      profile-parameterised width), distance field, `H = profile(d)`, normals = grad H in
      the corpus encoding (+Y-up, #8080FF outside). All 16 cells → the atlas. Acceptance:
      the atlas renders; a side-by-side vs the art (capture) shows registered bands.
- [x] Gate with `atlas_check`: flat/piece/normal-seam metrics ≈ 0 BY CONSTRUCTION
      (report them), relief within the healthy band and REGISTERED (the relief pixels sit
      in the art's bevel/face bands). Acceptance: numbers in `completed.md`.

## P2 — wire, ship, drill

- [x] `bin/art normal --analytic <kind>` → `atlas_geometry.py`, stamping
      `normal_engine=analytic`; non-analytic engines untouched. Acceptance: the command
      regenerates the smooth wall's `normal.l.0.png`; the stamp reads `analytic`.
- [x] In-game drill at zoom 1 + zoom 2 beside the torch: walls shade as continuous
      geometry with crisp, art-registered bevels. Acceptance: captures in
      `completed.md`; the user's eyes are the final oracle.

## P3 — the ControlNet spike (one kind: brick)

- [x] Spike the conditioning recipe on the ComfyUI box: `controlnet-union-promax` with
      the ANALYTIC normal (and/or the free height field as depth) conditioning + the
      brick diffuse → a candidate brick normal atlas. Acceptance: ONE candidate lands in
      the scratchpad with the recipe (graph JSON) recorded in the stream folder.
- [x] Gate the candidate: `atlas_check` consistency within 2× the smooth analytic's
      numbers AND relief ≥ the smooth baseline; a side-by-side capture vs the brick
      Marigold output. Acceptance: numbers + captures in `completed.md`; GO/NO-GO for
      the productionising follow-on recorded in `forks.md`.
