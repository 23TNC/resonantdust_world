# Todo — shadow-polish

_P0 pins each bug to a proven cause BEFORE fixing (both suspects are located but
unconfirmed); the user's eyes ratify P3. Design + the user's steer: [`README`](README.md)._

---

## P0 — reproduce + pin

- [x] Reproduce bug 1 live: park the view on the walking wolf near the torch, capture the
      flipped n/s shadow at a moment the wolf faces n or s. Acceptance: a capture in the
      folder showing the head/tail inversion, with the wolf's facing noted.
- [x] Pin bug 1's sign: a CPU mirror of `casterCoverNS`'s u0/mrot mapping for the
      captured configuration (wolf pos, torch pos, facing) predicting which frame end the
      shadow samples. Acceptance: the mirror reproduces the flip; the sign/flip term is
      named in `issues.md`.
- [x] Pin bug 2's path: at a texel where the wolf wears its shadow, read the fine
      receiver map + the hot/cold light words (`debugReadShadow` + a receiver probe) and
      name which path delivered the ground shadow (tile mode-2 claim, hot receiver
      mis-class, or a correction gap). Acceptance: the delivering path named in
      `issues.md` with the probe values.

## P1 — bug 1: un-flip the n/s cast

- [x] Fix the identified sign/flip in `casterCoverNS` (u0 mapping or the mrot/caster_flip
      selection). Acceptance: the CPU mirror now predicts the correct end; corridor↔brute
      identity 0 mismatches per class (`debugReadShadow` diff, corridor on/off).
- [x] Live drill: the wolf walking n and s past the torch — the shadow's head end tracks
      the wolf's head, sweeping e/w with the light's column crossing. Acceptance:
      captures at both facings in `completed.md`.

## P2 — bug 2: the wolf wins its own texels

- [x] Fix the receiver priority so wolf-drawn texels take the on-billboard result in the
      hot correction (the tile mode-2 path yields wherever a billboard covers; ground
      keeps the wolf's cast shadow beside it). Acceptance: the P0 probe re-read at the
      same texel shows the billboard path; cold bakes stay 0 during a walk (counter).
- [x] Live drill: the wolf walking through the torch pool — no self-shadow on the sprite,
      its ground shadow intact around it, tile lighting intact beside it. Acceptance:
      captures in `completed.md`.

## P3 — bilinear shadow sampling (user, 2026-07-30)

- [x] Manual 4-tap bilinear in the BLIT's lightmap read (the accumulators are INTEGER
      textures — no hardware filtering; decode each tap /QUANT then weight, honoring the
      hot class's negative deltas). Acceptance: shadow edges render smooth at zoom 1 + 2;
      a before/after capture pair of the same tree shadow edge in `completed.md`.
- [x] Guard the class seam: filtering must not smear ground lighting back ONTO billboard
      texels across the receiver boundary (the exact seam bug 2 fixes) — clamp or
      class-gate the taps if the drill shows halos. Acceptance: the bug-2 drill re-run
      under bilinear stays clean; fps ≥ 100 with the 4-tap blit (number recorded).

## P4 — the joint drill

- [x] All three together at zoom 1 + zoom 2: wolf circling the torch (n/s + e/w legs),
      walls + trees + wolf shadows coherent with smooth edges; fps ≥ 100 in the drill
      view. Acceptance: captures + the fps number in `completed.md`; the user's eyes are
      the final oracle.
