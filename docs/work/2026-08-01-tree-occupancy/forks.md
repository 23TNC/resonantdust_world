# Forks — 2×3 trees + tile occupancy

## F1 — square masters + letterbox over non-square frames {#f1}

A 2×3 tree could motivate non-square def frames, but the pow2-square master model is a
settled decision (aspect plumbing was REMOVED — texture-pow2-normalization) and the
whole record chain already recovers true proportions from the opaque bbox + subframe.
Chosen: `size 3` square drawn box, subject letterboxed 2×3, bottom-anchored. Cost: ~44%
of the canvas is empty margin at this aspect; revisit only if atlas pressure appears.

## F2 — spacing as a pure per-tile tournament {#f2}

Options: (a) post-pass over the generated zone (breaks per-tile purity, needs
cross-zone stitching); (b) anchor-grid + jitter (pure, but visibly grid-locked at low
jitter and still needs a conflict rule at high jitter); (c) local tournament — candidate
iff roll passes, place iff it beats every conflicting candidate, each neighbour's roll
recomputed from its own coordinates (chosen: pure, order-free, cross-zone-correct by
construction, and degrades to today's rule when the window is 1×1).

## F3 — occupancy derived, not stored {#f3}

Storing occupancy in a shard makes it a second truth that every mutation must keep
consistent forever. Derived-on-load + maintained-in-memory means the shard stays the
single truth (things × footprints) and a worker restart rebuilds the cache for free.
The pathfinder consumes a QUERY, not a table — if it later needs persistence or
cross-worker visibility, that is its stream's fork to open with this one's data.
