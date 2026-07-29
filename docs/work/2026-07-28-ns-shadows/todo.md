# Todo — ns-shadows

_Items tick in place; the box is the move. Design: [`README`](README.md). Visual items verify
at ≥3 zooms + during a transition, tab VISIBLE (hidden tabs freeze the chase — recorded
observation)._

---

## P0 · Record plumbing (the caster def)

- [x] VARIABLES.md: claim `billboard_data.A` — u16 caster_definition_id (16–31), u1
      caster_flip (15), u1 caster_valid (14), rest reserved. Acceptance: layout documented
      before code lands; `rd docs-check` green.
- [x] `billboardDataFor`: for rotations 0/2 resolve the EAST frame's def (same stem, drawn
      lod) through the existing `definitionFor` path and stamp A (valid=0 if unresolved,
      flip per D3). Acceptance: console probe shows a walking-n wolf's record carrying
      drawn AND caster defs.
- [x] Fast-path compare includes the A word, so a caster-def swap (zoom lod change) rewrites
      the record and cascades dirty. Acceptance: a zoom transition swaps the caster def and
      the billboard dirty fires (counter probe).

## P1 · The perpendicular card in the shadow walk

- [x] `buildCasters`: bucket rot-0/2 casters by the ROTATED footprint — centerline column
      (±1 pad), rows spanning the side frame's width centered on the anchor row (D2).
      Acceptance: corridor finds every texel brute finds on a rotated caster
      (`debugReadShadow` identity).
- [x] `casterCover` rot-0/2 arm: card plane x = centerline, no lean; s = n–s offset across
      the side width (flip per D3), t = height; ray–plane solve + the analytic penumbra
      interval on the rotated axis; silhouette sampled from caster_definition_id.
      Acceptance: point light east of a n-facing wolf → westward side-profile shadow;
      mirrored from the west.
- [x] `casterOne` gates for rotated casters: self-exclusion by id unchanged; the seen-face /
      near-band test gets a rotated-geometry equivalent (centerline vs receiver base);
      painter row stays the anchor row. Acceptance: no self-cast on the n/s wolf; its shadow
      climbs a tree standing east/west of it.
- [x] Fallback: caster_valid 0 or lod<4 → solid rotated quad (centerline × side width ×
      height). Acceptance: shadow present (blocky) before textures resolve.

## P2 · Verify

- [ ] Live drill: wolf walking n/s past the torch — the shadow sweeps e/w from the center
      line, flips side as the wolf crosses the light's column, and tracks at the hot-sync
      lockstep cadence. Acceptance: user's eyes in motion; captures at rest in
      `completed.md`.
- [x] Guards: zoom stability at ≥3 zooms + a transition; 120 fps at zoom 1 while walking;
      brute↔corridor bit-identity for rotations 0 AND 2. Acceptance: numbers logged in
      `completed.md`.
