# Deviations — art pipeline on 128 px tiles

_Log a deviation from the plan AT THE MOMENT of deviating, with the reason. "Less churn" is never
a reason._

None yet.

- **2026-07-28 · P2 item order.** Working P2.3 (read span from the corpus) **before** P2.2 (write it
  into the leaf). P2.2's acceptance — "conifer variant 0 shows span 2" — cannot be met without the
  reader, so as written P2.2 depends on an item below it, which the plan's own ordering rule
  forbids. Reason is a plan defect, not convenience; the items are otherwise unchanged.

- **2026-07-28 · I damaged `smooth/wall` during P2.7.** Running `art split biome-tile/default/smooth`
  to check the linked sidecar fell through to the legacy id-walk, which treated each map FILENAME as
  an id and created junk `<map>.l.0/1/diffuse.png` directories (`1.l.0/`, `1.s.0/`, `albedo.l.0/`,
  `albedo_marigold.l.0/`, `albedo_residual.l.0/`, `diffuse.l.0/`, `layers.l.0/`, `normal.l.0/`). The
  real leaf maps at the top were untouched. Cleaned up as part of the B1 migration below. Root cause
  is the same as B1(a): `smooth/wall` is a NAMED variant leaf, and the variant-level detection only
  recognised NUMERIC ones, so neither branch matched and it reached the legacy path.
