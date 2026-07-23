# Forks — emitter soft shadows

_Decision points + options + which we chose + why. Chronological._

---

## F1 · WHERE penumbra applies — quad-band vs silhouette multi-tap {#f1}

**2026-07-23 — RECOMMEND (B) silhouette multi-tap; confirm before P1.**

The shadow edge follows the SPRITE SILHOUETTE (surface B), which sits inside the projected quad.
So softening the quad boundary (A) doesn't soften the visible shape edge.

- **(A) Quad-band**: signed distance to the quad edge → `smoothstep(±w_world)`. Zero extra samples,
  but the silhouette hides the quad edge → penumbra only on the coarse outer blob. **Weak.**
- **(B) Silhouette multi-tap** *(recommended)*: sample surface B with a kernel of radius `w_world`
  (in atlas texels) → averaged soft coverage. Real penumbra — the shape edge blurs, emitter-scaled.
  Cost: N taps on the existing B sample (corridor-bounded); start 4–9, measure.
- **Hybrid** (later): quad-band coarse falloff + a cheap 4-tap silhouette blur, if B alone is too
  costly or too noisy.

Open question for the user: accept the multi-tap cost, or start with the cheap quad-band and only
go multi-tap if the penumbra reads too weak?

## F2 · Penumbra width model {#f2}

**2026-07-23 — proposed.**

`w_world = emitter_radius · (t·Zt)/(Lz − t·Zt)`, using the card-height fraction `t` already computed
for the silhouette inversion. Physically motivated (angular emitter size projected from card height
to ground): 0 at the base, `e·(k−1)` at the tip. Alternatives if it reads wrong: a simpler uniform
`w = e·k` (constant per caster, no `t` dependence — flatter but cheaper), or a tuned artist curve.
Refine visually in P1.

## F3 · Tap count / kernel {#f3}

**2026-07-23 — open until P1.**

4-tap (rotated) is the cheap floor; 9-tap (3×3) is smoother. A radius-scaled Poisson set trades
banding for noise. Decide by eye + perf in P1; the tap loop must be a constant bound (GLSL) with the
radius as data.

## F4 · Umbra: emergent vs explicit contact-darkening {#f4}

**2026-07-23 — default EMERGENT; explicit dial optional.**

The umbra (dark core, base-dark → tip-light falloff) **emerges** from the emitter multi-tap (B): the
`t`-growing kernel keeps coverage ~1 near the base and drops the peak with distance. No separate term
needed for physically-plausible behaviour. If the near-base darkness needs to be stronger/artier than
physics gives (RimWorld-style contact shadows), add an **explicit contact-darkening multiplier** —
`opacity = mix(1.0, contactBoost, 1 − t)` or a distance-from-base curve — as a tunable, default off.
Decide by eye in P1; keep it a dial, not a hardcode.
