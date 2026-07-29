# Cast shadows onto prims — attempt #3, in-family receiver input — 2026-07-24

> **RECONCILED 2026-07-28 (pawn-render P5): this plan's substance got BUILT — by other
> streams.** The in-family receiver input this folder proposed exists as the baked
> RECEIVER MAPS (coarse + fine, own dirty channel) in `shadowGather.ts`; the climb
> (`zElev = k·(baseY − y)`), ground projection, seen-face + light-side cone culls, and the
> corridor↔brute-shared `casterOne` are all LIVE (lightmap-resolution → standing-costs →
> lighting-feel line), and pawn-render extended receiving/casting to MOVER billboards
> (hot-class). The 0/8 todo below predates all of that — do not execute it. Kept for the
> design record; status: closed (delivered via successors).

_Component: [`client/webgl`](../../components/client/) · `game/viewport/`. Phases in [`todo.md`](todo.md);
decisions in [`forks.md`](forks.md); findings in [`issues.md`](issues.md). Supersedes the reverted
[`2026-07-23-shadows-on-prims`](../2026-07-23-shadows-on-prims/README.md)._

Make cast shadows land **on standing billboards** — climbing the sprite at the right height — instead of
the current placeholder (billboards take full light, no shadow) or the old flat/offset blob.

## What we already have, and what actually failed
Attempt #2 ([`2026-07-23-shadows-on-prims/completed.md`](../2026-07-23-shadows-on-prims/completed.md)) built
the whole thing and it was **correct at a fixed zoom** — corridor↔brute bit-identical, the climb/flat/binary
A/B all right. It was reverted for **one** reason: the receiver's `is-thing` + `base-row` came from the
**`zdepth` composite**, a `textile_slot` map (variable `slotPx` slot atlas), read by **recomputed world
coordinate** — which is not zoom-stable, so it drifted on zoom and corrupted even ground shadows
([map-compatibility](../2026-07-24-map-compatibility/README.md)).

**So the fix is narrow: replace ONLY the receiver-input source with a zoom-safe one; reuse the rest.** The
downstream math is known-good and lifts over verbatim:
- receiver elevation `z = sin65·(base_y − pixel_y)` (the ratified [world-geometry](../2026-07-23-world-geometry/README.md) model),
- projection to the ground point `G = L.xy + s·(P − L.xy)`, `s = Lz/(Lz − z)`,
- the two cone culls (user): **seen-face** — caster must be strictly SOUTH of the receiver row (else the
  shadow lands on the unseen back; also kills self-shadow); **light-side** — caster must sit between the
  light and the receiver,
- `casterOne()` shared by the corridor + brute walks ⟹ the P6 bit-identity holds.

## The one new thing — a zoom-safe receiver mask
The gather must answer, per texel, in a **zoom-stable** way: *is a standing prim drawn here, and what is its
base row?* The hard-won rule ([map-compatibility](../2026-07-24-map-compatibility/README.md)): a shader may
address a map by recomputed world coordinate ONLY if that map is a **contiguous** `cols·R × rows·R` grid —
never a `textile_slot` composite. So the receiver mask must come from a **contiguous / in-family** source
([forks F1](forks.md#f1)); the leading option is to **bake the receiver depth into a `textile_unit` map**
(same family + resolution as `shadow-cold`, aligned texel-for-texel), which the gather then samples by world
coordinate safely — i.e. the exact input attempt #2 wanted, re-homed to the correct map family.

**This does NOT wait on the map-compatibility F1 decision** — it sidesteps it by choosing an in-family
source outright, which is safe under either branch.

## Non-goals / guard rails
- **Zoom stability is a P0 acceptance test, not an afterthought.** Every phase verifies at ≥3 zoom levels +
  during a zoom transition (the failure mode that reverted attempt #2). A fixed-zoom screenshot proves
  nothing here.
- Do not re-introduce any world-coord read of a `textile_slot` composite (`albedo`/`normal`/`surface`/
  `zdepth`) from the gather or lighting pass.
- Warm (mover) receivers stay out of scope for the first cut (cold receivers only), same as attempt #2.
</content>
