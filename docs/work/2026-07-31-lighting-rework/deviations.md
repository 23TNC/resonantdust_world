# Deviations — lighting + shader rework

_Log any departure from [`todo.md`](todo.md) AT THE MOMENT of deviating, with the reason._

_None yet._

## D1 — P0's harness acceptance cited a stale baseline (2026-07-31)

**Plan:** "reproduces the stripped baseline of **0.045 ms/frame** within its own spread."

**Reality:** 0.045 was the strip's **P1** figure. Its P2 and P4 took the floor to **0.028** by deleting
`moverDirty`'s per-frame record rewrites and the cursor light — both found after this plan was written.

**Resolution:** the harness was checked against the *current* floor (0.035 / 0.028 across two runs,
each inside the other's spread), which is what the acceptance was actually for. Logged rather than
quietly re-baselined, because a plan number silently edited to match a measurement is how a harness
gets trusted for the wrong reason.

## D2 — the summed map is `RGBA32F`, not `RGBA16F` (2026-07-31)

[F1](forks.md#f1) costed the sum at "8 MiB at `RGBA16F`". It has to be `RGBA32F`: the exactness
acceptance needs every value a slot can produce represented **without rounding**, and 8 slots × 1023
quantisation levels needs 13 mantissa bits where FP16 has 11. FP32's 24 covers it with room to spare.

**Cost: 32 MiB instead of 8.** Still far under the 99 MiB the strip freed, and it buys the property the
whole slot/sum split exists for — an update that can be undone exactly.

## D3 — a light update is three draws, not two (2026-07-31)

**Plan:** "changing one light rewrites one slot and **one blended draw**, not the whole map."

**Built:** three draws — withdraw `−slot`, rewrite the slot, deposit `+slot`.

**Why.** The two-draw form (emit `new − old`, then write `new`) predicts what the hardware will store,
and prediction is not exact: the slot keeps `RGB10_A2` rounding the delta did not apply. Quantising the
prediction in software still disagreed in the last bit (measured 0.0039, one full quantum). Reading the
stored value cannot be wrong about it.

The acceptance's *intent* — "not the whole map" — is fully met: one slot of eight, and no other slot or
region is touched. The next item's acceptance (bit-exact add/remove) is unreachable in two draws, so
this trades a draw for the property the phase exists to deliver.

## D4 — the gather is checked on OCCLUSION, not caster identity (2026-07-31)

**Plan:** "0 differing texels between the corridor walk and an exhaustive search."

**Reality:** where several casters block the same ray, *which* one is found is arbitrary — the walk
takes the first along the ray, brute the first in scan order, and **both are complete answers** under
this design's binary occlusion. Identity differed on 1079 of 131 072 slots and always will.

**Built:** the check compares whether each slot is occluded **at all** — which is the property a
shadow is actually drawn from. That is **0 differing**, exactly.

The identity count is still reported, because a *large* swing in it would mean the tie-break changed
even though the shadows did not, and that is worth being able to see.
