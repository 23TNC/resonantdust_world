# Deviations — cast shadows onto prims

_Logged at the moment of deviating from the plan. Never for "less churn"._

---

## D-1 · F1 receiver mask: inline aerial SCAN instead of a separate `textile_unit` bake {#d1}
**2026-07-24.** [`forks F1`](forks.md#f1) leaned toward baking a `textile_unit` receiver-depth RT. While
executing I chose the **inline** variant instead: the gather determines `is-thing`/`base-row` at each texel
by scanning the EXISTING caster buckets a few rows SOUTH (a prim's drawn billboard extends north of its
base, so the covering prim's base sits at/south of the texel row) and testing each prim's UPRIGHT silhouette,
taking the frontmost (southmost) cover. Why: it needs **no new RT, no new pass, and no new bucket set** —
the caster buckets already hold exactly the standing prims, and the whole thing stays in-family (data
texture, zoom-safe) with one early-out (all scanned buckets empty → ground). Still F1's intent (in-family,
never a `textile_slot` world-coord read); just cheaper to land. If the per-texel scan proves too costly, the
fallback is aerial buckets (7 tests/texel) or the bake RT — but measure first.
</content>
