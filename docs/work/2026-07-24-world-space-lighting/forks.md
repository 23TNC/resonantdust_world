# Forks — world-space lighting

_Decision points + options + which we chose + why. Chronological._

---

## F1 · Full un-projection vs inline anisotropic correction {#f1}
**2026-07-24 — OPEN (settle in P0/P1).** Two ways to get the world-3D light vector per texel:
- **(a) Inline correction** — keep working in screen/world-units, and build the light vector as
  `(Δx, Δy·k_ns, Zterm)` where `k_ns` is the derived N–S factor and `Zterm` folds the light height (+ the
  point's fictional elevation for billboards). A few extra ALU; no un-projection matrix.
- **(b) Full un-projection** — un-project each texel to its true 3D world position, un-project the light to
  3D, subtract. More general (handles any camera), heavier.
Lean **(a)** — the tilt is a fixed `65°` constant, so the correction is a couple of scalars; (b) is more than
this needs. Revisit only if the camera model gains freedom.

## F2 · The N–S foreshorten factor — derive, don't guess {#f2}
**2026-07-24 — OPEN (P0's core output).** Candidates: `cos65` (in-plane foreshortening of a plane at 65° to
the view), `sin65` (the fictional-height coefficient), or a combination once the light height enters. This is
the exact `2·tan65`-vs-`sin65` class that misfired on world-geometry — so it is DERIVED from the ratified
model and **simulation-checked** before code, not asserted here.

## F3 · Where the normal N·L fix lives — this stream or normal-tilt {#f3}
**2026-07-24 — split at the dot product.** [`normal-tilt`](../2026-07-24-normal-tilt/README.md) owns baking
the normals into the world frame (the surface operand). THIS stream owns the **light-direction** operand
(world-3D) and the `N·L` itself in `LIGHT_FRAG`/the blit. They must agree on the world frame (axes, `+y`
sign) — cross-link so the two don't drift. Neither absorbs the other.

## F4 · Additive relief vs clamped Lambert {#f4}
**2026-07-24 — OPEN (P3 look-call).** Today: `relief = 1 + gain·dot(N.xy, dir)`, `light = ambient + irr·max(
relief,0)` — additive, can exceed 1, so a light-facing bump reads brighter than flat (the user's
bright-spots-amid-shadow worry). Once `N·L` is world-correct we can switch to a **clamped Lambert**
(`irr·max(0, N·L)`, never brighter than flat) for physical honesty, or keep the additive gain as a
deliberate art amplifier. Decide by eye at P3; it only makes sense to decide AFTER the direction is correct.
</content>
