# Todo — lighting steady-state gating

_Every phase ends with a `__framecost` row beside the baseline AND `__lightexact` still
bit-identical. Stances: [`README`](README.md)._

---

## P0 — instrument the chain

- [ ] Add a per-pass GPU cost breakdown to `__framecost` (recordSync / gather / receivers
      / slots / sum, EXT_disjoint_timer where available, wall-clock fallback).
      Acceptance: a printed table at N=16 static naming where the 9.55 ms goes.
- [ ] Pin the change signals each pass depends on (record dirty set, emitter signature,
      window origin, mover motion) in `issues.md` — the gate spec. Acceptance: the table
      lists, per pass, exactly what must be unchanged to skip it.

## P1 — skip-when-unchanged

- [ ] Gate the whole chain: when the P0 signals are all unchanged, issue no lighting
      passes for the frame. Acceptance: N=16 STATIC `__framecost` within ~2× of the
      0.104 ms unlit floor; the lit image pixel-identical before/after gating.
- [ ] Prove the gate opens correctly: move one light, one mover, and the window; each
      re-lights within one frame (no stale pools, no shadow lag on screen).
      Acceptance: captures + `__lightexact` bit-identical.

## P2 — per-light scissored updates

- [ ] Route single-light changes through the existing withdraw/write/deposit delta path,
      scissored to that light's slot rectangle, instead of the full-window recompute.
      Acceptance: N=16 with ONE moving light ≈ the N=1 moving cost, not the N=16 cost.
- [ ] Feed mover-driven changes through the same route (a mover entering/leaving a
      pool touches only that pool's lights). Acceptance: wolf soak at the fixture with
      `__framecost` ≈ static outside crossings; image correct during crossings.

## P3 — upload + pan increments

- [ ] Shrink prim/light/presence uploads to changed rows (dirty-rect texSubImage).
      Acceptance: upload bytes/frame counter at N=16 static = 0; moving ≪ full-texture.
- [ ] Re-light only the entering strip on a window pan (the toroidal maps keep the rest).
      Acceptance: pan `__framecost` ≪ full recompute; no seams or stale strips on screen.

## P4 — the numbers

- [ ] The final table: N ∈ {1, 8, 16} × {static, one-mover, panning} beside the baseline,
      soak-verified (wolf running ≥ 10 min, no drift — `__lightexact` before/after).
      Acceptance: the table in `completed.md`; the README's idle target met or the miss
      explained.
