# Issues — lighting standing costs

_Problems hit, candidate solutions, which we chose and why._

## I1 — pre-existing corridor↔brute divergence: 398 words, light near the window edge {#i1}

(2026-07-27, found during P1 identity verification — **not introduced by this stream**, proven by
bit-exact hashes: baseline and P1 produce identical brute (2575314166) and identical corridor
(2077624216) readbacks, but the two walks differ from each other.)

Fixture: `__torch()` lights the standing billboard at tile **(15, −1)** — one row NORTH of the
window's row band — reach 8. Cold shadow RT readback: **398 of 524 288 words differ** between
`setCorridor(false)` and `setCorridor(true)`, against 7 600 nonzero words (~5 % of the shadow).

Suspected class: the light's reach box crosses the window edge, where the brute box walks
absolute-tile buckets through the toroidal `foldTile` — tiles outside the window alias onto
in-window slots, so brute can read a bucket the corridor (which walks only the light→texel
segment) never visits, or vice versa. The corridor identity has previously only been re-proven on
HOT-class lights well inside the window (plane-intersection P4: "0 differing of 39 083" — a hot
readback ~13× smaller than this cold one).

Not chased here (this stream must not change WHAT is computed — its contract is exactly that these
hashes stay fixed). Needs its own investigation: reproduce with a light fully interior to the
window (expect 0), then at the edge (expect divergence), and decide which walk is wrong at edges.
