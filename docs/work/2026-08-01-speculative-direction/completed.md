# Completed — speculative direction

## 2026-08-01 · P0 — the pops, pinned

Instrumented every facing transition (`[facing] old->new src d=(rendered delta) r=(pos)`)
and captured a wolf soak + a diagonal human trip. Three pop classes named with receipts
in [I1](issues.md#i1) — all `src=spec-walk`: the diagonal-chase stair-snap
(`1->0 d=(0.036,0.036)`), the post-reseed full disagreement (`2->3` while rendering
south), and the e/w-first walk leading the render (`3->1` on a dominantly-south step).

## 2026-08-01 · P1 — motion-derived facing

**The rendered delta decides**: `tick` derives facing from `(rx−prev, ry−prev)` — the
motion the eye actually follows — with a dead-zone (0.002 tiles), a minimum hold
(150 ms), and an axis-DOMINANCE margin (1.3×) added mid-verification: a pure diagonal
glide (Chebyshev steps advance both axes equally) is a knife-edge under float noise, and
without the margin the sprite oscillated once per hold interval — with it, neither axis
dominates and the current facing persists. `walkGreedy` no longer returns a facing at
all; the arm-time INITIAL AIM applies the same e/w-first rule inline (`src=aim`).
Server facing applies only from rows that ARRIVE at rest (F1) — the landing row never
turns the pawn, a mid-motion row steers position only, and a genuinely turned standing
pawn now re-applies (closing the stale-rest gap). `p.facing` grep-clean; typecheck
clean.

## 2026-08-01 · P2 — verified on screen

**The trip log** (fixture, wolf soak + human drills): ~50 s of wolf trips produced FIVE
transitions total — `src=motion` only at genuine axis turns (`3->2 d=(-0.001,-0.009)`,
a dominantly-north leg), `src=server-rest` only between trips, ZERO diagonal
oscillation (previously one flip per 150 ms). The mid-trip RE-ORDER drill: an east trip
superseded 2.5 s in by a west order logged exactly one `1->3 src=aim` at the
superseding intent and one `3->2 src=motion` at the final leg — no pops at hop
boundaries, reseeds, or landings.

**The records inherit**: probed mid-glide — the wolf moving west carried
`rotation=3, cast_type=1` with live fine positions (x 1626.25→1616.19); the rotation
lane IS the mover's speculative facing, so the shadow card turns with the sprite. The
ns card case (`cast_type 2` at facings 0/2) rides the same lane, verified by the
mechanism rather than caught mid-north-glide.
