# Deviations — 2026-07-21-shadow-bitfield

_Where the code departs from the plan. Row: date · plan · code · why · status._

## D-2 · 4-corner projected quad instead of the 5-triangle fan — 2026-07-21 (open, user-requested)

- **Plan/design:** `shadow-projection` + `design/shadows.md` specify the **5-triangle projected-silhouette
  fan** (7 points, incl. the `dA`/`dB` base-spread). The gather ported that (P4-silhouette).
- **Code:** the gather predicate now builds a **standard 4-corner projected billboard rect** (2 triangles,
  plain quad UVs); `dA`/`dB` are **dropped** (treated as 0) — still extracted + passed but unused in
  `inShadow`. `SUV`/`TRI` fan constants removed.
- **Why:** the user reported **diagonal-line artifacts** from the fan (the `dA`/`dB` base-spread slivers +
  the center-anchor triangles); the 4-corner quad removes them. Browser-verified: diagonals gone, clean
  tree-shaped shadows. (New: a thin horizontal ground-contact line where flat bottom edges align across a
  tree row — inherent to the quad, flagged to the user.)
- **Status:** open. If the quad sticks: the `billboard_definition_data` `dA`/`dB` fields (VARIABLES.md) become
  vestigial → reclaim or note; the `shadow-projection` design's 5-triangle model needs updating. Kept the
  `dA`/`dB` data + params so a return to the fan is a one-function revert.

## D-1 · Implement gather-visible-first, then layer presence/dirty — 2026-07-21 (open)

- **Plan (todo phase order):** P2 `light_presence_cold` → P3 dirty → P4 gather.
- **Code (build order):** P4-core first (shadow-cold RT + gather over **all** lights, billboard-quad
  region, **full recompute each frame**, + the overlay decode) → **visible** result; then P2 presence
  (cull) and P3 dirty (incremental) layered on as **non-visual optimizations** that don't change the
  picture; then the silhouette-mask refinement.
- **Why:** dirty (P3) is an invisible optimization — building it first is a lot of unverifiable code.
  Gather-first yields a testable picture. Strong reason (testability), not churn-avoidance. Phase
  *numbers* in todo/completed still map to the plan; only the execution order differs.
- **Correction (2026-07-21):** presence (P2) is **not** invisible — it's a *semantic* cull (a light
  only needs shadow bits where it illuminates), so it **clips shadows to each light's radius box**, a
  correct + expected picture change. Only **dirty (P3)** must be a true no-op on the picture.
- **Status:** RESOLVED 2026-07-21. P3 landed as a picture no-op (static identical to P2; pan world-locked).
  Gather-first paid off — the SQUARE bug and the presence-clip semantics were caught against a live
  picture before dirty gating went on top. Execution order (P4-core → P2 → P3) diverged from the plan's
  numbering; both reach the same place.