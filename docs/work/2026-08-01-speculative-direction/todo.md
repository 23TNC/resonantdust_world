# Todo — speculative direction

_Facing follows the motion; the server's facing applies only at rest. Stances:
[`README`](README.md)._

---

## P0 — pin the pop

- [ ] Instrument facing transitions (`console.debug`: old→new + source spec/chase/server)
      and capture one wolf trip + one right-click human trip. Acceptance: the observed
      pops named in `issues.md` with their source line each.

## P1 — motion-derived facing

- [ ] Derive in-motion facing from the RENDERED per-tick delta `(rx−prev, ry−prev)` in
      `MoverLayer.tick`: dead-zone tiny deltas, dominant axis wins, ties keep current.
      Acceptance: a gliding pawn's facing matches its on-screen direction every frame of
      a logged trip.
- [ ] Add flip hysteresis: a facing holds ≥ a minimum interval (constant, ~150 ms)
      before it may change again; sub-eps motion never turns. Acceptance: a diagonal
      wolf trip logs no facing flips at tile stair-steps.
- [ ] Demote `walkGreedy`'s facing to the INITIAL aim only (the first frame of a spec,
      before any rendered delta exists). Acceptance: grep shows `p.facing` consumed
      nowhere else; typecheck.
- [ ] Apply server facing ONLY at rest (no spec, chase gap ≈ 0) — and DO adopt it there:
      an authoritative row that turns a standing pawn re-applies its visual. Acceptance:
      turn a resting pawn server-side → it turns; a mid-motion row never turns it.

## P2 — verify on screen

- [ ] The wolf soak drill: ≥ 3 trips watched at the fixture, plus a right-click human
      trip with a deliberate mid-trip re-order. Acceptance: no facing pop at hop
      boundaries, reseeds, or landings; captures + the transition log in
      `completed.md`.
- [ ] The records inherit: the prim `rotation` lane + `cast_type` follow the speculative
      facing (shadow card flips with the sprite, ns↔ew switches at turns). Acceptance:
      `__zprobe`/record probe mid-trip shows rotation matching the rendered facing.
