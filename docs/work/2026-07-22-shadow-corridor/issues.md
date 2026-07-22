# Issues — 2026-07-22-shadow-corridor

_Known limits, accepted risks, and open sub-decisions. Distinct from [`forks.md`](forks.md)
(settled choices) and [`blockers`] (none yet)._

---

## I-1 · Slot-keyed output demands stable presence slots — OPEN (design rule)

The 4-bit coverage is keyed by **presence slot (0–7)**, not global light. So: (a) **any** change to a
tile's presence set must **dirty** the tile, or a persisted slot describes a light no longer in it;
and (b) slot assignment must be **stable** — keep a present light in its slot, fill only *freed* slots
with newcomers. Distance-sorting the 8 every frame would reshuffle slots and dirty tiles that didn't
need it. Rule for P5: stable slotting, dirty on genuine enter/evict only.

## I-2 · The 64-cap failure is accepted (and self-masking) — ACCEPTED

After the height cull, the only way to whiff is **64 casters whose bands reach the texel but which
are all transparent at its exact projected point**, missing a 65th that would hit. Accepted,
because it's self-masking:

- **Per-texel + decorrelated** — each of the 256 texels/tile samples its own projected point, so a
  miss is a scattered stray, never a blank tile (you'd need all 256 to independently whiff).
- **It's *under*-shadowed, not unshadowed** — you still accumulated penumbra/near-misses, just didn't
  reach the true value.
- **It only occurs in dense caster pileups** — which are *dark* — so the error is a faint brightening
  on an already-near-`0xF` tile. The condition that triggers it is the condition that hides it.

Since it only touches the rare dropout, the **64 cap is a safe quality dial** (raise to shave it on
strong hardware, lower on weak). Revisit only if a specific shot shows it — unlikely.

## I-3 · Shadow-only opaque bbox — DEFERRED

Tightening the sprite frame to its opaque bounding box would cut transparent-margin sample waste and
shave I-2 further, but tightening the **shared** frame drags every other map (albedo/normal/surface)
along. Deferred. When revisited: a **shadow-only bbox** (extra def fields the gather uses to clamp its
sample + derive W/H, other maps keep the full shared frame) likely avoids the cross-map churn — the
tight opaque bbox *is* the visible silhouette, so it stays consistent, and `basePad` already does the
vertical half. Additive metadata, not a geometry change. Not blocking; the failure it addresses is
already in the "imperceptible" bucket.

## I-4 · Sub-tile smoothness vs tile-crossing-only dirty — OPEN (quality call)

The cast reads the 1/16-tile anchor, so a mover changes its shadow **every frame it moves**, not just
on tile crossings. Dirtying only on crossings → **tile-quantized (snapping)** hot shadows; the
projected tip snaps by up to ~a tile's worth (amplified) at each crossing. If snapping is acceptable
(short/subtle shadows) → crossing-only is a big win. If smooth is wanted → re-dirty a mover's
footprint every frame it moves (cheap: sub-tile motion is small, so it's just the current footprint
of the few things that moved). Decide during P8.

## I-6 · A caster in multiple corridor tiles is cast once — RESOLVED (design rule for P6)

With **whole-caster buckets** ([F5](forks.md#f5)), a caster spanning multiple tiles — a **wide
object along the corridor** or a **≤1-tile straddler** mid-move — is bucketed into each, so the
corridor may encounter it more than once. (This subsumes the old straddler-only framing; slicing,
which would have seamed at every column boundary, is gone.)

- **Binary (P1–P5): no action** — testing the whole caster twice is idempotent (same bit, break-on-
  first). The seamless behaviour is free here.
- **Coverage/penumbra (P6+): dedup, keyed on `caster_ref`** — else the caster's coverage accumulates
  twice (a doubled *region* wherever the corridor runs along the caster's length, plus doubled
  penumbra edges).

  **Chosen: check the toward-P neighbours, no stored field** (user, 2026-07-22 — supersedes the
  abandoned `tile_seq`, "defer toward the spatial anchor", "defer toward the previous corridor tile",
  and "all earlier-canonical bbox tiles" attempts). The bucket entry stays a **plain `u16` prim ref**.
  Aim the check **backward along the corridor** (toward `P`, the texel being solved), **not** at the
  anchor:

  > **cast iff none of the ≤3 neighbours in the toward-`P` octant is in `S = prim ∩ corridor`** (bbox
  > test + `onCorridor`, the same predicate that defines the sweep). Else defer.

  **No stored field, no neighbour-list read.** `S` is a connected path *along* the corridor, so a
  tile's predecessor in `S` is always one of its ≤3 toward-`P` neighbours; the unique `P`-most tile of
  `S` is the sole caster. Works for **any prim shape** — no dependence on size or anchor position.
  - **Why toward-`P`, not toward-the-anchor** (the key fix): the anchor's location is unrelated to the
    corridor's direction. A solid ≥4×4 prim with an **anti-diagonal** corridor (perpendicular to the
    anchor) makes each middle tile look NW (empty) while its real `S`-neighbours are NE/SW → **multiple
    casters**. Toward-`P` always points along `S`, so it can't miss the predecessor. (The earlier
    rejected attempts each fail a sub-case: previous-tile breaks on non-consecutive thick-corridor
    visits; toward-anchor breaks as above; canonical-order works but is O(bbox), not O(1).)
  - **Bound = 3, never N−1:** contiguity means you check *adjacent* tiles, not all earlier. Corridors
    are **1-tile wide** ([F9](forks.md#f9)), so `S` is a 1-wide path with a **unique `P`-most tile** →
    a flat **3-neighbour** toward-`P` check, **no tie-break**. (A thick corridor could present a flat
    2-wide front → two `P`-most tiles → tie, needing ~5 with a perpendicular check; moot at width 1.)
  - **CPU maintenance (two independent streams):** (1) write the prim's **x/y every frame it moves**
    (feeds the projection); (2) update **bucket presence on tile-boundary crossings** — add a tile
    when the bbox starts covering it, drop one when it leaves (a mover: A → A+B straddle → B). Wide
    prims touch N tiles on move (fine — they'd have to anyway).

  **Alternatives considered (both work, both cost registers):** a **consecutive-step window** (dedup
  vs the previous step's ≤8 refs, ~4×u32 carried), and a **bounded 15-log** (dedup memory is capped by
  coverage resolution — ≤15 contributors before `0xF`-and-break — not the 64 cap; ~8×u32 live +
  dynamic-index scan). The toward-`P` check matches their completeness at **zero** register cost and
  **no neighbour read**, so it wins.

Note the earlier **clip-to-tile-bounds** idea is rejected for the same reason slicing was — any
partition seams at the boundary (dark overlap / bright gap under float rounding). Whole-caster never
partitions, so there is no seam to manage; dedup only prevents the double-*accumulate*.

## I-7 · Some casters miss their shadow — OPEN (debug, noticed 2026-07-22)

At 3–6 lights, `/overlayRT` shows **some in-reach trees casting no shadow**. Prime suspect: the
**base-line bucketing is a single tile row** (`buildCasters` buckets a caster only into
`floor((y+height)/SQUARE)` × width cols) intersected with a **1-tile-thin corridor** — a P→L march can
cross the caster's *position* at a row just off its base row and never read its bucket, so the caster
is never tested. Candidates when we debug: bucket the caster into a small **neighbourhood** (base row
± 1, or its full footprint), and/or confirm the corridor Bresenham vs the base row alignment. Distinct
from the accepted F9 corner-graze — this is systematic, not a rare edge. Isolating one light and
walking a known tree↔light pair on `/overlayRT` (colours now match the gizmos, so attribution is
clean) is the way in.

## I-5 · Hot↔cold migration must be symmetric — OPEN (design rule for P8)

Promotion (cold→hot on any data change) is defined; the **settle** (hot→cold when a thing stops) must
be its exact inverse — dirty **both** fields on the settling thing's tiles — or hot only ever grows
and the "cold rarely recomputes" advantage bleeds away. Define settle = inverse of promote in P8.
