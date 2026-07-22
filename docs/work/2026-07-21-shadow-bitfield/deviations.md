# Deviations — 2026-07-21-shadow-bitfield

_Where the code departs from the plan. Row: date · plan · code · why · status._

## D-1 · Implement gather-visible-first, then layer presence/dirty — 2026-07-21 (open)

- **Plan (todo phase order):** P2 `light_presence_cold` → P3 dirty → P4 gather.
- **Code (build order):** P4-core first (shadow-cold RT + gather over **all** lights, billboard-quad
  region, **full recompute each frame**, + the overlay decode) → **visible** result; then P2 presence
  (cull) and P3 dirty (incremental) layered on as **non-visual optimizations** that don't change the
  picture; then the silhouette-mask refinement.
- **Why:** presence + dirty are invisible optimizations — building them first is a lot of unverifiable
  code. Gather-first yields a testable picture, against which presence/dirty must be a no-op. Strong
  reason (testability), not churn-avoidance. Phase *numbers* in todo/completed still map to the plan;
  only the execution order differs.
- **Status:** open (will close when P2+P3 land and the picture is unchanged).