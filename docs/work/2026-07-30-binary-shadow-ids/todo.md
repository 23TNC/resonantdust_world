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

## P2 — The id map ([F7](forks.md#f7): 8 slots, in the existing texel)

- [x] Drop `PRES_SLOTS` to 8 and confirm the gather's light loop, the presence fetch and the CPU mirror all follow the constant. Acceptance: one presence word fetched, not two; loop bound 8; no literal 16 left behind.
- [x] Re-encode the shadow texel as 8 x u16 caster ids, `onBillboard << 15 | billboardIdx`, sentinel for none. Acceptance: the coverage bit is GONE — occlusion is `id != sentinel` at every reader.
- [x] Record the winning caster's id from the walk into its slot. Acceptance: sampled ids resolve to real casters whose footprint plausibly covers the texel.
- [x] Verify and measure. Acceptance: corridor↔brute 0 differing, scene renders correctly, and the headline re-measured — the 8-slot loop should beat P1's 7.11 ms at N16.

## P2b — [F8](forks.md#f8): self-positioning casters + the u20 reference

_Added 2026-07-31 ([D2](deviations.md)). F8 postdates this plan and is the PREREQUISITE for P3/P4 —
it is what dissolved [B2](blockers.md). Ordered so each item leaves the tree renderable._

- [x] `VARIABLES.md` — `billboard_data` R gains a ROOT/CHILD split (root = `region|zone|tile|unit`, child = `parent_id|tile|unit`) and A gains `u1 child`. Acceptance: the layout block reads like `prim_data`'s and the "No `resolved_zone`" paragraph is replaced by the reason it is no longer needed.
- [x] `VARIABLES.md` — re-spec the STALE `shadow-cold` block (it still says 14 × u9 coverage; P1/P2 made it an id map). Acceptance: the doc describes the v6 header+ids layout, and no `u9`/`14 slots` text survives.
- [ ] `coldShadowData.ts` root path — write the full resolved position into R. Acceptance: `r.pos` goes in whole; the mirror readback shows region/zone bits set for a standing tree.
- [ ] `coldShadowData.ts` child path — set `child = 1` in A for a CARRIED billboard only. Acceptance: a pawn head reads child 1, a tree reads child 0, verified from the mirror.
- [ ] `shadowGather.ts` — decode a root caster with `decodePos` (absolute, no `ref`); keep `resolvedTilePos` for children. Acceptance: corridor↔brute 0 differing, and the scene renders unchanged — this item alters no shadow, only how a position is recovered.
- [ ] Shadow RT to 2 px per texel — single attachment, 2× width, NOT MRT ([I6](issues.md): MRT hung twice). Acceptance: the page loads and renders; no hang, verified in Chrome before anything is written to px 1.
- [ ] `GATHER_FRAG` writes the v6 pair: header px (4 channels × 2 lights × `u4 set`) + id px (8 × `u16` in-set id). Acceptance: `set == 0` IS the empty sentinel (VARIABLES: set 0 = the global sentinel), so no magic id is needed.
- [ ] Update every reader to the 2-px stride — `accumulateLights`, the overlay, `debugReadShadow`, the identity diff. Acceptance: `debugReadShadow` returns a decodable `(set, id)` pair and the overlay still matches the gizmo colours.
- [ ] Verify and measure. Acceptance: corridor↔brute 0 differing, a page load confirming no regression, and the headline re-measured against P2's 6.08 ms — the 2× RT costs bandwidth, so a rise here is expected and must be quantified, not waved through.

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
