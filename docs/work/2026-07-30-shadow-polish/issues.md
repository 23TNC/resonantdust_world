# Issues — shadow-polish

_Defects found during execution. P0's pins land here FIRST (the cause named with probe
values, before any fix). Known inputs: the npc wolf must be running (`FORCE=1 bin/sim run
npc` if its container exited); measurements need a FOREGROUND tab (background freezes
rAF); the hot class may hold NEGATIVE deposits (the cold-light delta) — probes must read
signed._

## Pre-pin (code audit, 2026-07-30): the user's fine-offset hypothesis CONFIRMED

**Where fine offsets ARE applied (all verified):** every GLSL record-position decode
carries the sub-unit lanes — `casterCover` (l.424), `receiverCover` (l.533),
`billboardNormal` (l.602), `casterOne`'s Cb (l.661); the fifth fetch (`billboardHot`) is
position-free. CPU-side, `moverDirty`'s rects and the base-line registration derive from
the record-SNAPPED prim position (hot-sync P3's authority), so registration and dirty
regions track at 1-px grain. Lights evaluate at the fine texel's TRUE world position
(`P`, 64/tile) against `resolvedPos` light positions.

**The asymmetry the user found:** the SHADOW is not fine at all. The shadow RT is
UNIT-resolution (`SHADOW_TEXELS = TEXTILE_UNIT` = 16/tile — one texel per unit), the
gather evaluates it at each COARSE texel's center, and the lighting pass upsamples it
with `fcC = fc / FINE` (integer floor, `texelFetch` NEAREST — l.870/873/1018): all 16
fine texels inside a shadow texel share one word, with ZERO fractional consideration.
So the wolf's sprite + lighting move at 1-px grain while its shadow moves in 8-px unit
steps — the "clips forward" stair on the leading edge is exactly this quantisation, and
the dark squares on the wolf are plausibly UNIT-sized shadow texels straddling the
silhouette (their center sampled a ground point beside the wolf, carrying its own cast
shadow, then nearest-upsampled onto sprite texels) — possibly compounding with the
receiver-classification edge cases already suspected. The live P0 probe still
distinguishes the two contributions per square.

**Consequence for P3:** bilinear is UPGRADED from cosmetic to the missing fine-offset
consideration — the 4-tap must interpolate the coarse shadow PER LIGHT SLOT with the
fine texel's fractional position (unpack each neighbour's u9 slot coverage, weight,
then use), which is precisely "applying the same fine offset consideration when
sampling the coarse shadow textile."

## P0 pin · Bug 1 — the ns card is anchored at the BASE-CENTRE, not the sprite's center line

REPRODUCED (controlled drill — npc stopped, the wolf driven by `moveEntity` orders; the
n-facing wolf parked at (103,57), dominant light NE): the wolf's shadow stub lies
ENTIRELY SOUTH of the wolf, hanging off its tail — not spanning its body. That geometry
rules a pure u-axis mirror OUT as the primary cause and points at DISPLACEMENT: the ns
card spans `A.y ± ws/2` where A = the record's base-centre = the SPRITE BOTTOM (the tail
tip for an n/s sprite) — but the design says the card stands on the sprite's CENTER LINE,
and for a top-down n/s sprite the drawn body IS the ground footprint, so the card must
span `[A.y − ws, A.y]` (equivalently: center at anchor − ws/2). The CPU mirror of the
observed configuration matches: base-centre anchoring predicts the stub at ~57.7–58.3
(observed); center-line predicts it alongside the body at ~56.4–57.6 (expected). The
same half-length displacement infects the REGISTRATION rows (`r0/r1 = floor((ay ∓
ws/2)/SQUARE)` centered on ay — must become `[ay − ws, ay]`). Whether a residual u-axis
head/tail mirror ALSO exists is indeterminate from the stub (silhouette unreadable at
this scale) — the P1 fix drill re-checks head-tracks-head explicitly. (The e/w tilted
card anchors correctly — a side-view sprite's base line IS its bottom.) Captures live in
the session transcript (WebGL canvas readback is blank without preserveDrawingBuffer —
noted for future drills).
