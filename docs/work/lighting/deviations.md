# Deviations — lighting

_Plan-deviations logged at the moment they happen. "Less churn" is never a reason (see the
`deviations-file` memory). Format: what the plan said → what we did → why._

---

## D-1 · P4 cold shadows: GPU lane bake, not a CPU `active` map

> **⚠ CORRECTED 2026-07-19 — this deviation measured against the wrong baseline.** D-1 compared the
> build to the *written* plan (a CPU `active` map), and judged the 3-light cap "the same ceiling we
> already accept for dynamic." But the **actual** plan (spoken, not in the doc) was the **inline
> per-pixel light sweep with no shadow map** — which has *no* channel cap. So both the build AND the
> written plan it was checked against had already diverged from intent; the cap was never acceptable
> for cold. The correct decision is now [F7](forks.md#f7); the `shadow-cold` build below is the
> **interim** to be replaced in the P4 rework ([todo.md](todo.md) P4, gated on [B3](blockers.md#b3)).
> Lesson: when a "design-locked" reference doc and the stated intent conflict, that's a fork to
> surface — not a thing to resolve silently in the doc's favour.

**Plan** ([`todo.md`](todo.md) P4, from the old game): a CPU-built 32-bit/pixel `active` map per
cold light, rect-aware, folded into the `cold_lightmap` bake. _(Itself already a divergence from the
inline-sweep intent — see the correction above.)_

**Did:** baked the cold shadows GPU-side into a derived `shadow-cold` SquareCache composite
(R/G/B = cold light 0/1/2), by reusing `projectCaster` — the SAME billboard-silhouette shear the
pending P3 dynamic scatter uses. The cold lightmap bake samples that composite and subtracts the
shadowed light's term.

**Why:**
- **One shear, two tiers.** `projectCaster` is already the shared cold+dynamic primitive (its
  docstring calls this out). A separate CPU rasteriser for cold would duplicate the silhouette
  projection in a second language/space and drift from the GPU path.
- **Rect-boundary crossings** — the reason the old game went CPU per-pixel — are handled for free by
  baking each square in WORLD space over ALL casters within `radius+300` (not per-rect clipping). A
  caster outside the square still projects its shadow into the square if it reaches.
- **Amortized already.** The cold lightmap bake is a per-dirty-square GPU pass; adding a shadow
  sub-pass to it keeps everything on the same toroidal/dirty/apron machinery. A CPU map would need
  its own dirty tracking + upload.

**Cost / caveat:** ≤3 cold shadow-casting lights per square (R/G/B lanes), vs the CPU map's 32-bit
capacity. Matches the P3 scatter's 3-lanes-per-map constraint (the GPU-can't-merge-bitfields limit
in the README), so it's the same ceiling we already accept for dynamic. A 4th+ cold shadow-caster
casts light but no shadow. Revisit if content authors >3 overlapping shadow-casting cold lights in
one square.
