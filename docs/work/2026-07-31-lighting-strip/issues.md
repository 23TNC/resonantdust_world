# Issues — strip the lighting + shadow system

_Problems hit, candidate solutions, which we chose and why. The first entries are **inherited**: the
structural findings that motivate the strip, carried here with their evidence so the re-think argues
with data rather than recollection._

## I1 — The shadow texel is 128 bits, and every extension needs more

**Evidence:** [`2026-07-30-binary-shadow-ids`](../2026-07-30-binary-shadow-ids/issues.md) I6, I6b,
I6c, I8.

A shadow texel is one `uvec4`. Eight lights per tile fills it exactly at `u16` per slot — which is
why 8 was chosen, not because 8 lights is the design ceiling. The trouble is that a **caster
reference is `u20`** (the data texture is 16 `u16`-addressable sets, so a record needs `u4 set | u16
in-set id`), and 8 × 20 = 160 bits.

Every route to the second px costs something that was discovered rather than planned:

| route | what happened |
|---|---|
| MRT, two `rgba32ui` attachments | **hung Chrome twice**, never root-caused. Bisected to a single attachment writing a constant — still hung. `SquareCache.mrtScratch` runs 4 `rgba8unorm` attachments fine, so it is this *pass*, not the driver |
| 2×-wide RT, one attachment | legal and never hung — but the gather rasterizes both px, so **the walk runs twice**, and the walk is ~87 % of the pass |
| a derive pass reading the id RT | cheap and legal, but at N=1 the header is a pure function of the ids — it carries no information |

And the payload the second px was going to hold is, today, **constant**: every caster the walk
considers is a billboard (the loop skips anything else outright), so `u4 set` is 6 for every occupied
slot and 0 for every empty one — which the id lane's sentinel already encodes.

## I2 — `max` keeps HOW MUCH and discards WHICH

**Evidence:** [`2026-07-26-moving-lights`](../2026-07-26-moving-lights/README.md) F7;
[`2026-07-30-binary-shadow-ids`](../2026-07-30-binary-shadow-ids/issues.md) I1.

The gather accumulated coverage with `max(cov, casterCover(...))`. That stores a *number*, so the
identity of the occluder is destroyed at the moment it is computed. When the fine-edge refine needed
that identity back, it had to re-run the **entire `walkShadow` corridor** per fine texel — DDA, per-tile
bucket fetches, `casterCover` over the caster slots — to rediscover something the coarse pass had
just thrown away. It was deleted at **9.29 ms of a 10.88 ms pass — 85 %**.

The id map fixed exactly this, and immediately hit [I1](#i1). The pattern is worth naming: the cost
was the **search**, not the evaluation, and the search existed only because the storage layout could
not hold an identity.

## I3 — Penumbra was computed and discarded, for an unknown number of streams

**Evidence:** [`2026-07-30-binary-shadow-ids`](../2026-07-30-binary-shadow-ids/completed.md) P0
histogram.

The map carried `u7`, later `u9`, coverage for emitter-based soft shadows. A live histogram of the
shadow RT found it **perfectly bimodal: 31 155 shadowed slot-samples, zero partial.** Geometry
predicted a ~21-texel gradient. So the analytic λ interval and the chord→area curve were evaluated
per caster per light per texel and the result was flattened somewhere downstream.

The user had been saying the shadows were hard-edged; the reply had been that penumbra was "live",
asserted from reading `radius 0.35` and seeing the code path exist. **The histogram settled it in
five minutes and the assertion was wrong.** Recorded here because the lesson generalises past
penumbra: a rendering claim is only worth what the screen says.

## I4 — Z-order is resolved per pixel, in the hottest shader in the frame

The display blit compares `zdepth.B = 0x80 | (baseRow & 0x7f)` per pixel, under `if (wcov > 0.0)`,
to composite warm over cold. That works, and it is why movers can be layered without re-baking cold.

It also means **layering is a fragment cost, not a draw order**: adding a layer means adding a
comparison to every pixel of the frame. The conventional alternative — one draw per layer, ordered —
was discussed and not taken. A re-think should take the question seriously rather than inherit the
answer.

## I5 — The differential was designed, allocated, and never wired

`DIFFERENTIAL_WIRED = false`. The lighting pass draws with `blend: "none"` and **recomputes** rather
than differencing. `coldShadowPrevRT` and `hotShadowPrevRT` are allocated at 2 MiB each, never
written and never read — **4 MiB idling since they were created.**

The unlock was real and was recorded: `max` → `any` made the stored value order-independent, which is
precisely what an invertible update needs. It was deferred to keep a measurement clean and never came
back. If the replacement wants incremental updates, this is the piece that was 90 % built.

## I6 — Eight lighting/shadow work streams are open against a system being deleted

`docs/work/README.md` carries `binary-shadow-ids`, `shadow-polish`, `lighting-feel`,
`light-budget`, `lighting-standing-costs`, `tile-lighting`, `analytic-wall-normals` and
`marigold-linked-normals` — several `open`, describing work on code that P2 removes.

Left alone they read as live commitments. P4 closes them to `../archive/` as **superseded by the
strip**, not as delivered — the distinction matters, because a future reader deciding whether an idea
was tried and rejected or simply overtaken needs to be able to tell.

Note that the two Marigold/normals streams are **art pipeline**, not renderer: per
[F4](forks.md#f4) their output survives the strip, so they are candidates to stay open. P4 decides
per stream rather than sweeping.

## I7 — WebGL timer queries do not retire reliably in a backgrounded tab {#i7}

Recorded 2026-07-31 so nobody rebuilds this rig from scratch. It is a **measurement** problem, not a
renderer one, and it is what ended P0 ([D1](deviations.md)).

The debug tab runs `document.hidden`, which the shadow streams already knew (their harness is
tick-driven because rAF is suspended — measured **0 rAF calls in 10 s**). What was not written down is
that `EXT_disjoint_timer_query_webgl2` results need an **event-loop turn** to become available, and in
that same backgrounded tab `setTimeout` is throttled to roughly **1 s per call**.

| drain strategy | result |
|---|---|
| `setTimeout(4)` polling | **works** — but each poll costs ~1 s, so a 3-repeat run blows the 45 s CDP evaluate budget |
| driving extra `tick()` frames | **0 of 810** queries retired — GL command submission is not an event-loop turn |
| `gl.finish()` | **0 of 810** — a GPU sync is not sufficient either |
| `MessageChannel` yields (unthrottled) | partial — **740 of 810 missing**; the yields return too fast to let the GPU process retire anything |

So the working recipe is the slow one, and it caps a run at roughly one repeat per evaluate call.

**What to do instead, if per-pass GPU time is ever needed again:** measure one repeat per call and
aggregate across calls, or foreground the tab. Do not spend the session making the fast paths work —
they fail by returning *plausible partial data*, which is worse than failing loudly. Note the same
shape as the capture bug in `completed.md`: the wrong method here does not error, it under-reports.
