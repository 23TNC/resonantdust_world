# Forks — lighting correctness

## F4 — P0 exposed a missing phase: live integration comes before the named fixes

P0's pin ([I1](issues.md#i1)) showed the rework's lit path is a hand-driven debug
harness: off by default, records built once (racing frame streaming), movers recordless,
content lights unread. Verifying bbox/silhouettes/z against hand-minted snapshots would
verify the harness, not lighting — so the plan gains **P1b (wire to the live scene)**
between the reach split and the bbox work. Ordering kept: P1's reach-lane split first
(it changes the writer signatures P1b then feeds), P1b second (everything downstream
verifies against a LIVE lit world), then the named fixes. Recorded as a fork rather than
a deviation: the plan is amended before the work, not departed from silently.

## F1 (plan-time) — the reach/intensity bit split: `u4 + u6`, not `u5 + u5`

**Chosen:** `prim_data.B` bits 0–9 become `u4 reach (bias +1 → 1..16 tiles) | u6 intensity`
(64 levels across the 0..4 overbright range, step 0.0625).
**Why 16 caps reach:** reach is the measured cost dial (the old fps table: three movers at
reach 16 → 18 fps, at 8 → 120 fps; cost compounds as walk-length × claimed-texels² ×
overlap), and 16 tiles is the ceiling every headline number is quoted at. Encoding 31 would
invite authoring the exact value the system is known to choke on.
**Rejected:** `u5 reach | u5 intensity` — 32 intensity levels is a 0.125 step, visible in
slow fades, to buy reach the content has never used. Recorded so flipping is one edit if a
long-throw light (a lighthouse, a sun shaft) ever becomes content.

## F2 (plan-time) — N·L bakes into the per-light slots, not the display blit

**Chosen:** the slot pass resolves the texel's receiver (it already walks presence for the
refine), samples that receiver's normal quadrant, and multiplies N·L into the light's
contribution — the old FINE-lightmap model. The summed map inherits shading; the display
stays one fetch.
**Rejected:** N·L at display time — it would re-resolve the receiver per screen pixel per
frame (the exact cost F1-of-the-rework split the maps to avoid) and would break the summed
map's meaning (it would no longer be the displayable light).
**Consequence accepted:** a receiver whose normal frame changes (a mover turning) must
dirty its texels' slots — the same dirty movement already raises.

## F3 (plan-time) — the old-vs-new verdict compares structure + fresh like-for-likes, never
stale absolutes

The old system's recorded numbers are ~4× low (rework I10: `gl.finish()` never synced), and
the old code is deleted — so "old 0.508 ms vs new X" would be fiction. The verdict instead:
(a) fresh measurements of the NEW system, moving lights, everything on; (b) the structural
argument for why the old system COULD NOT reach the same regime (its own measured internal
ratios — the 9.29 ms re-search inside a 10.88 ms pass — survive the 4× scaling because both
sides of a ratio scale together); (c) the honest costs. This is the strongest claim the
evidence supports.
