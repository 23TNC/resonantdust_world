# Forks — shadow-polish

_Decisions resolved in-stream, with rejected options. The user pre-authorized the
overwrite strategy for bug 2 ("if we cannot overwrite please let me know") — the README
answers that we CAN (the hot-correction model already replaces); if P0 disproves it, the
alternative strategy goes BACK TO THE USER as a blocker, not a fork._

## F1 · Execution reorder: P3's bilinear lands BEFORE P2's re-probe (2026-07-30)

Two reasons. (1) The pin's arithmetic: the observed squares are UNIT-sized — the coarse
shadow texel (8 px) — and the def's tight box turned out to EQUAL the drawn opaque width
(the "narrower than drawn" phrasing in the pin overstated; the hard-0.5 contour gap is
sub-texel and the conservative 4-tap already covers ~1 texel), so the unit-quantised
NEAREST upsample is the dominant square-maker and P3's per-slot fractional filter is the
direct fix. (2) The live world is CONTESTED right now (the user building walls; a master
clock reset 65k→454 mid-drill made my tab's anchor stale) — batching every remaining
drill into one P4 pass minimises disruption and re-checks P2's acceptance under the
filtered shadow. If squares persist at silhouette edges after P3, the P2 residual fix is
storing SOFT coverage in the fine receiver word's spare bits and scaling the hot carve.
