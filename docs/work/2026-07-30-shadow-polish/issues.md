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
