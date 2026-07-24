# Forks — cast shadows onto prims

_Decision points + options + which we chose + why. Chronological._

---

## F1 · Where the zoom-safe receiver mask comes from {#f1}
**2026-07-24 — OPEN (P0; the crux of attempt #3).** The gather needs per-texel `is-thing` + `base-row` in a
form it can address by world coordinate WITHOUT drifting on zoom. The reverted attempt read it from the
`zdepth` `textile_slot` composite — the exact incompatible lookup. Options:

- **(a) Bake a `textile_unit` receiver-depth map.** A small pass rasterises each standing prim's drawn
  billboard silhouette into a contiguous toroidal RT at `TEXTILE_UNIT` (16/tile), aligned texel-for-texel
  with `shadow-cold`, storing `is-thing` + `base-row`. The gather samples it by world coord — SAFE, because
  it is the same contiguous family/resolution as `shadow-cold` (world→texel is fixed at every zoom). This is
  literally attempt #2's input, re-homed to the correct family, so the rest of the (verified) pipeline drops
  in unchanged. *Cost:* one extra per-prim bake pass. **LEAN — smallest, most direct fix of the actual bug.**
- **(b) Aerial caster buckets, tested inline.** Add a second per-tile bucketing keyed by each prim's DRAWN
  bbox (the current caster buckets key by the ground BASE line, which is south of where the billboard is
  drawn, so they can't answer "is a prim drawn at this upper texel"). The gather reads the aerial bucket,
  inverts the texel into each prim's frame, samples the surface silhouette inline → `is-thing`/`base-row`,
  no extra RT. *Cost:* a second bucketing + per-texel silhouette tests in the already-hot gather; more
  surface area, easier to get subtly wrong. Reuses `casterCover`-style inversion.
- **(c) Conform the composites to a contiguous `textile_square`, then sample the existing `zdepth`.** Only
  legal if [map-compatibility F1](../2026-07-24-map-compatibility/forks.md#f1) picks the "conform" branch —
  a large, deferred re-plumb. **Not now** (the user deferred that investigation).

Recommend **(a)**: it isolates the fix to the one thing that was wrong (the input's map family) and reuses
the verified elevation + cone-cull + corridor machinery verbatim. Revisit (b) only if the extra bake proves
too costly.

## F2 · Reuse attempt #2's cone-cull math, or redo it? {#f2}
**2026-07-24 — REUSE.** The elevation, projection, and the two cone culls (seen-face + light-side) were
verified correct at a fixed zoom (corridor↔brute bit-identical; climb/flat/binary A/B all right). They are
independent of how the receiver mask is obtained. Port them verbatim onto the F1 input; do not redesign.
The seen-face cull (caster strictly south of the receiver row) is the fix for the user's back-face bug and
must be preserved exactly.
</content>
