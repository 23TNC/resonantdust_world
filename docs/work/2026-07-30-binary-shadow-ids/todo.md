# Plan — binary occlusion + stored caster ids

_The plan for the life of the stream. Items never move; `[x]` IS the move. Context in
[`README.md`](README.md)._

**Acceptance for the whole stream.** Every phase must hold these:

- **corridor↔brute identity = 0 differing texels** (`__corridor(false)` + a shadow diff). Binary changes
  WHAT is stored; corridor and brute must still agree on it.
- **Every perf claim comes from the P0 harness**, same fixture, same repeats. A change smaller than the
  measured spread is not a result.
- **No silent capability loss.** Anything binary occlusion gives up is written down when it is given up,
  not discovered later.

**Fixture:** area1, zoom 1, the tick-driven harness (rAF is suspended in a backgrounded tab) — draws
discriminated by render target, `orbitPhase` reset so phase derives from the frame index, fixed frame
count, 12 warm-up frames dropped, 3 repeats reporting mean and spread.

## P0 — Checkpoint and baseline

- [x] Record `b35103a3` as the A/B reference SHA in `completed.md`; the tree is already clean, so no checkpoint commit is needed. Acceptance: the SHA is written down and `git status` is clean at the moment of recording.
- [x] Histogram the live `coldShadowRT` u7 coverage values ([F1](forks.md#f1)). Acceptance: a distribution — all-0-or-127 means penumbra is already dead; a spread means it is computed and lost downstream. Recorded either way.
- [x] Build the light-scaling harness: N orbiting lights, all reach 16, zoom 1, measuring gather + lighting ms. Acceptance: cost rises with N and with reach, so the harness responds to workloads we control.
- [x] Measure the headline BEFORE: the largest N whose gather + lighting fits in **8 ms**. Acceptance: a single number, plus the ms at N and at N+1 so the boundary is visible.
- [x] Sweep the supporting matrix: reach 4/8/12/16 × N 1/2/4/8/16, zoom 1. Acceptance: a table with per-cell mean and spread, saved to `completed.md`.
- [x] Capture a zoom-1 crop of a conifer shadow edge as the before-image. Acceptance: the stepped edge is identifiable, at a spot reproducible after the change.

## P1 — Binary occlusion

- [x] Make `casterCover` return 0 or 1 and delete the analytic λ interval, the chord→area curve and `frac`. Acceptance: the function has no penumbra arithmetic left; `git diff` shows deletions, not a flag.
- [x] Collapse per-slot storage to 1 bit of coverage, leaving the other 7 bits of the byte defined and documented. Acceptance: the lighting pass reads the new encoding; no `>> 1u` / `127.0` remnants.
- [x] Replace `max(cov, cc)` with an `any`-style early exit out of the caster loop. Acceptance: the walk stops at the first occluder; a shadow diff before/after shows only intended differences.
- [x] Verify and measure. Acceptance: corridor↔brute 0 differing, the P0 matrix re-run, and the delta attributable to deleted work rather than changed geometry.

## P2 — The id map

- [ ] Allocate the id map at shadow resolution, two px per texel for 16 × `u16` ([F2](forks.md#f2)). Acceptance: ~4 MiB per class, confirmed by the live resident-bytes walk.
- [ ] Track the winning caster's prim id alongside the occlusion test and write it per slot. Acceptance: sampled ids resolve to real casters whose footprint plausibly covers the texel.
- [ ] Confirm nothing else changed. Acceptance: the shadow RT itself is bit-identical to P1 — the id map is additive.

## P3 — Incumbent early-out

- [ ] Test the stored id before walking; on a hit, skip the corridor entirely. Acceptance: exact under binary — corridor↔brute still 0 differing, since any occluder is a complete answer.
- [ ] Instrument the hit rate and log it. Acceptance: a percentage for a moving light and for a cold re-bake, so the saving is attributed rather than assumed.
- [ ] Re-measure the headline and the matrix. Acceptance: the light count at 8 ms, compared against P0 and P1.
- [ ] Confirm a recycled prim id cannot corrupt the result. Acceptance: the incumbent is TESTED not trusted, demonstrated by forcing a stale id and showing the walk still runs.

## P4 — Fine placement in the lighting pass

- [ ] Use the stored id in `LIGHT_FRAG` to run one `solveCentre` + `sampleCard` per fine texel. Acceptance: the shadow edge resolves at 64/tile; the before-image crop is visibly finer.
- [ ] Read the 2×2 id neighbourhood so the edge extends across coarse boundaries ([F4](forks.md#f4)). Acceptance: no coarse step on the LIT side of the edge, which one-id-only cannot fix.
- [ ] Gate the refine to coarse texels that actually straddle an edge. Acceptance: interior and fully-lit texels do no refine work; the gate's selectivity is a measured percentage.
- [ ] Measure the refine's own cost. Acceptance: ms attributable to the refine alone, against the README's 0.05–0.15 ms estimate — correct the README if it disagrees.

## P5 — The verdict

- [ ] Re-run the ENTIRE P0 matrix on the finished build. Acceptance: same fixture, same repeats, table beside the before table.
- [ ] State the headline: moving lights at reach 16, zoom 1, inside 8 ms — before and after. Acceptance: both numbers, and the ms at the boundary N.
- [ ] Re-measure resident bytes. Acceptance: the id map's real cost, and whether freeing 7 coverage bits let it shrink ([F3](forks.md#f3)).
- [ ] Record what binary cost us. Acceptance: a side-by-side crop and an honest sentence on what soft shadows would have looked like had they worked.

## P6 — The unlocked follow-on (record, do not build)

- [ ] Write up that `any` restores order-independence, which is what `DIFFERENTIAL_WIRED` needs, and that the prev RTs are already allocated and idle. Acceptance: a note in `issues.md` naming the flag and the 4 MiB — a successor picks it up without re-deriving it.
