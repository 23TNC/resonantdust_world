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

## F5 · Image-space blur → AREA-LIGHT emitter sampling {#f5}

**2026-07-23 — SUPERSEDED (F1's image multi-tap) by area-light sampling.** The delivered image-space
multi-tap (F1 option B — blur the projected card silhouette in atlas-uv space) read as a **flat
billboard translated + skewed**: a homography of one flat card is always going to look like the card,
not a grounded shadow. Two tells — a hard projected-quad boundary with inward-only blur, and the tree's
branch detail surviving (capped 12px / 9-tap) all the way to the tip.

**Replaced by AREA-LIGHT emitter sampling** (`casterCover`): the light is a disk of radius `emitter`
(units) at height Lz; for each of 16 sub-lights we re-invert P → card (s,t) and sample the HARD
silhouette, averaging. Grounds the shadow by construction — a silhouette edge at height z casts to a
ground point that shifts with the sub-light by `∝ z/(Lz−z)`: a **z≈0 contact edge doesn't move** (hard
attached base), a **high edge moves a lot** (soft tip, detail dissolving). The penumbra is now
"projected based on light + prim location" (the user's framing), not an image blur.

**Constraint that shaped it:** the gate stays the **HARD projected quad** (not dilated), so the occluder
set is unchanged and the **corridor↔brute identity holds** (verified 0 mismatches, lights frozen). A
dilated gate (to add the outward feather beyond the silhouette extremes) broke identity — the corridor's
segment walk isn't proven to reach occluders of the dilated region. **Follow-up:** widen the corridor
pad by the penumbra width + re-prove sufficiency to earn the outward feather. `__emit(px)` tunes the
radius live (default 20px = 5 units); F3's tap-count/F4's umbra now emerge from the 16 emitter samples.
