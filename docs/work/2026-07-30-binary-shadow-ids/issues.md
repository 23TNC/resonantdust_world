# Issues — binary occlusion + stored caster ids

## I1 — `max` is the reason the refine was expensive, and the reason it can be cheap

Worth stating once, plainly, because it is the whole stream in a sentence.

The shadow value is `cov = max(cov, cc)` over every caster the corridor visits. **`max` keeps HOW MUCH
and discards WHICH.** So the deleted F7 refine had no way to sharpen an edge except to re-run the entire
search — DDA corridor, per-tile bucket fetches, `casterCover` across 8 caster slots — purely to
rediscover an identity the coarse pass had already computed and thrown away.

Measured then: **9.29 ms of a 10.88 ms pass, 85 %, for ONE light.**

That cost was never the evaluation. It was the search. Storing the winner deletes the search and leaves
one `casterCover` — which the plane-intersection work has since made ~3.5× cheaper.

**The 85 % figure is not today's number** and should not be quoted as one. It was taken at
`FINE_RATIO = 8` (64 fine texels per coarse); today it is 4 (16 per coarse), a 4× smaller multiplier, on
a gather that is 3.2–3.8× faster per caster test. Anyone re-deciding this should re-measure rather than
inherit the number.

## I2 — `any` restores order-independence, which unblocks the differential

`DIFFERENTIAL_WIRED = false` in `shadowGather.ts`. The lighting draw is `blend: "none"` — dirty rects are
**fully recomputed**, nothing is subtracted. The whole point of the differential is that a light's old
contribution can be reproduced **exactly** and negated, which `LIGHT_QUANT`'s integer quantisation exists
to guarantee.

`max` over an order-dependent caster set was never a problem for that on its own — but an **incumbent
early-out** would have been, because "whichever occluder we happened to find first" is not reproducible.

Binary occlusion removes the objection entirely: with `any`, the stored value does not depend on which
caster was found or in what order. **The early-out and the differential stop being in tension.**

Also already paid for: `coldShadowPrevRT` and `hotShadowPrevRT` are allocated unconditionally in the
resize, but the snapshot blit that fills them sits inside `if (DIFFERENTIAL_WIRED && shadowPrevRT)`. They
are **never written and never read — 4 MiB of ping-pong buffers idling**, waiting for exactly this.

Deliberately out of scope ([F5](forks.md#f5)) so the headline measurement attributes cleanly. Recorded
here so a successor does not re-derive it.

## I3 — Stale "u9" comments; the real encoding is `u7 | u1`

Several comments in `shadowGather.ts` describe the shadow slot as `u9` coverage. It cannot be: 16 slots ×
9 bits = 144 bits, and the texel is a 128-bit `uvec4`. The authoritative line is the `PRES_SLOTS` doc:

> each a `u7 coverage | u1 on-billboard` byte in the 128-bit shadow-cold texel (slot i at channel `i>>2`,
> bits `(i&3)·8` — 16 × 8 = 128 exactly)

confirmed by the read in `LIGHT_FRAG`: `((... >> shK) & 0xFFu) >> 1u ... / 127.0` — shift off the flag
bit, normalise by 127. So coverage today is **128 levels**, not 512.

Both I and the user reasoned from the `u9` comments during design. P1 rewrites this encoding anyway;
whatever replaces it must be documented in ONE place, and the stale duplicates deleted rather than left
to mislead the next reader. The same file also carries `// lights 0–6` / `// lights 7–13` on the presence
fetches, which describe a v2 layout that `tileSlot`'s own header says v3 retired.

## I4 — Why penumbra collapses to binary at `radius 0.35` (open, not chased)

P0's histogram found **0 partial values in 31 155 cold shadow samples** — the map is perfectly bimodal.
Yet the same machinery produced **61 % partial** earlier in this session at `radius 2.0`, and the
geometry predicts a ~21-texel gradient at 0.35 (`11.2 u × 26.2/(40−26.2)`). So the code CAN produce soft
values; something collapses them at the shipped emitter size.

**Most likely: the λ-interval clamp.** The analytic penumbra solves `u(λ) = u0 + λ·Dslope` at `u=0` and
`u=1` and clamps the roots to the emitter's extent `[-1,1]`. `Dslope = perp.x · (1 − invk) · invW`, so it
shrinks with the emitter radius and grows with `1/W`. When `Dslope` is small relative to the card width,
both roots land far outside `[-1,1]`, the clamp returns either the whole range or nothing, and `frac` is
exactly 1 or 0 — no intermediate state reachable.

**Not investigated**, because it changes nothing this stream builds: binary is being adopted either way,
and P1 deletes the arithmetic in question. Recorded because it is the sentence a successor trying to
RESTORE soft shadows needs — the machinery was not missing or mis-plumbed, it was **clamped**, and the
lever is the emitter-radius-to-card-width ratio rather than anything in the solver.

Cheap confirmation if anyone wants it: histogram again at `radius 2.0` and at `radius 0.35` on the same
scene. Two numbers, ten minutes, and I4 is either confirmed or replaced.

## I5 — The 8 ms budget and the per-tile slot cap land in the same place

P0's headline is **15 moving lights at reach 16 inside 8 ms**. `PRES_SLOTS = 16` is the number of lights
one tile can hold, and at reach 16 **every light covers the whole 512-slot window**, so every tile
carries all N. A 17th light would have nowhere to go.

So today the perf ceiling (≈15) and the architectural ceiling (16) are **the same number**. That is a
coincidence of the current operating point, but it changes what success looks like for
[P5](todo.md):

**The headline metric as posed cannot exceed 16.** If this stream halves the gather, the result will not
be "30 lights" — it will be N16 sitting at ~4 ms instead of 8.3, i.e. **headroom**, not count. Reporting
only "lights at 8 ms" would understate a real win as "15 → 16, +1".

**P5 must therefore report both:**

1. **ms at N16, reach 16** — the headroom number, which is unbounded and is where a gather win shows.
2. lights-at-8ms — kept for continuity with P0, understood to saturate at 16.

Raising `PRES_SLOTS` past 16 is not free and is not in scope: it is exactly the 128-bit shadow texel
budget ([I3](#i3)), 16 slots × 8 bits. Binary coverage frees 7 of those 8 bits per slot, so *more slots*
is one of the things the freed bits could buy — noted here rather than planned, because
[F3](forks.md#f3) already defers the freed-bit question to P5's measurement.

## I6 — P2 attempt 1 locked up Chrome; reverted. MRT on the gather is not a free change

**What was tried.** Extend `coldShadowRT`/`hotShadowRT` from one attachment to **three** — attachment 0
the coverage texel as before, 1 and 2 the caster id map (16 slots x u16 = 256 bits = two uvec4s) — and
have `GATHER_FRAG` write all three, with `walkShadow` reporting the winning caster through a new
out-param.

**What happened.** The page loaded and then the renderer stopped responding: screenshot capture timed
out, then CDP itself timed out at 30 s, and the user confirmed **Chrome locked up**. Reverted to the
P1 commit (`c3ec08f8`); tree clean, no id-map code remains.

**What is NOT the cause.** The tree typechecked, and the four early-return paths were all fixed to write
all three attachments before loading (an unwritten attachment under `drawBuffers` is undefined, and
`rmode == 0` is the common path) — so this was not a naive undefined-attachment mistake. It is also not
memory: three 512x256 rgba32uint attachments is 6 MiB per RT, 12 MiB for cold+hot, against 405 MiB
already resident.

**Original hypothesis (SUPERSEDED by the I6b bisect below) — the gather's write bandwidth tripled.** The gather runs one fragment per shadow
texel with a heavy per-fragment loop, and it was already the dominant pass (6.19 ms at N16). Tripling
the bytes written per fragment on a draw that is already the longest in the frame is a plausible way to
trip the GPU watchdog / TDR, which presents exactly like this — a hang rather than a black screen or a
shader-compile error.

**Not confirmed.** A lock-up destroys the evidence, and I did not get a console read or a link-status
check before it went unresponsive. The next attempt must gather that evidence BEFORE it can hang.

**How to approach P2 again — smaller steps, each independently loadable:**

1. Add **one** extra attachment carrying 8 slots (u16 x 8 = one uvec4), not two. Load. If that alone
   hangs, the cause is MRT-on-the-gather itself and the id map needs a different home entirely.
2. Explicitly check `getProgramParameter(LINK_STATUS)` and the info log for the gather program on boot
   and log it, so a compile/link failure is distinguishable from a hang next time.
3. Consider writing the id map in a **separate pass** over the shadow RT rather than as extra
   attachments on the gather — it costs a second draw but leaves the hot pass's bandwidth untouched.
4. Consider the 6 reserved bits in the slot byte ([F3](forks.md#f3)) for a narrower id after all: no new
   attachment at all, at the price of capping ids at 64 per class-window. F3 deferred this on
   correctness grounds; a hang changes that calculus and it deserves re-deciding rather than assuming.

**P2 is left OPEN and unticked.** Nothing about P0/P1 is affected — those are committed, measured and
independently valuable (8.30 -> 7.11 ms at 16 lights).

## I6b — BISECT RESULT: any second colour attachment on the gather hangs the renderer

I6 named two candidate causes for the lock-up — extra write bandwidth, and register pressure from the
eight `uint` accumulators attempt 1 held live across the 16-iteration light loop. The bisect isolates
them, and the answer is neither of the ones I ranked first.

**The bisect.** ONE extra attachment instead of two. The shader writes a **constant**
(`uvec4(0xffffffffu)`) to it at every exit — no accumulators, no read-modify-write, no per-slot logic,
half the added bandwidth. Everything else identical to P1.

**It hung exactly the same way.** Page loaded, then the renderer stopped responding; CDP screenshot
timed out at 30 s. Reverted; tree clean at the P1 commit.

**Therefore:**

- **NOT register pressure.** The bisect added zero live registers. Ruled out.
- **NOT the accumulators.** Same. Ruled out.
- **NOT write bandwidth in any interesting sense.** One extra `rgba32uint` attachment on a 512x256
  target is ~2 MiB per frame of extra writes on a draw already costing 5.5 ms. That is not a plausible
  watchdog trigger on its own, and it still hung.
- **NOT GLSL validity.** `Program` throws on `COMPILE_STATUS`/`LINK_STATUS`, so a bad shader raises a JS
  exception rather than hanging. It compiled and linked.

**What is left: MRT on the gather pass itself is pathological on this driver.** The renderer reports
`ANGLE (NVIDIA GeForce RTX 2080 Ti, Direct3D11)`, so every WebGL2 call is translated to D3D11. An
integer-format multiple-render-target write from a fragment shader with this gather's control flow is a
plausible place for that translation to fall off a fast path. **Not proven** — a hang leaves no
evidence, and proving it would need a driver-level trace this stream does not justify.

**Consequence for the plan.** [F2](forks.md#f2) chose "two extra attachments on the shadow RT" as the id
map's home. That option is now **closed**, and with it the assumption underneath P2-P4: that the winner
id could ride along with the coverage write for free. Every remaining route has to get the id out of the
gather WITHOUT a second attachment:

1. **Pack the id into the existing texel.** Binary coverage freed bits 2-7 of each slot byte — 6 bits,
   64 ids. [F3](forks.md#f3) rejected this on aliasing grounds when a second attachment was available;
   it is now the only zero-attachment option and must be re-decided on its merits. A tile-LOCAL id
   (which of the tile's 8 caster slots, plus a small tile offset) may fit 6 bits where a global prim id
   cannot.
2. **Widen the existing attachment's format.** One `rgba32uint` is 128 bits; there is no wider integer
   format in WebGL2. Dead end.
3. **A second gather pass** writing ids to its own single-attachment RT. Costs a full second walk, which
   is precisely the search P2 exists to delete. Self-defeating — and this supersedes the "separate pass"
   suggestion I made verbally, which was wrong for this reason.
4. **Abandon the id map**; keep P1's binary win and close the stream.

**P2 remains open and unticked.** P0/P1 are unaffected and committed: 8.30 -> 7.11 ms, 15 -> 16 lights.

## I6c — MRT is NOT broken in this engine; it is broken on THIS pass

Checked after I6b, and it sharpens the diagnosis considerably. `SquareCache` line 381 allocates
`mrtScratch` with **four** colour attachments:

```
formats: ["rgba8unorm", "rgba8unorm", "rgba8unorm", "rgba8unorm"]
```

That is the G-buffer bake, and it runs every frame a mover moves — 4 simultaneous attachments, working
fine, on the same driver that hangs when the gather is given a second one.

So "MRT on ANGLE/D3D11 is unusable" is **wrong**, and I6b overstated it. Three things differ between the
working case and the failing one:

| | bake (works, 4 attachments) | gather (hangs, 2) |
|---|---|---|
| format | `rgba8unorm`, 4 B/texel | **`rgba32uint`, 16 B/texel — INTEGER** |
| shader | a textured quad blit | **nested loops: up to 64 DDA steps x 3 tiles x 8 caster slots, each running casterCover** |
| draw cost | sub-millisecond | **the frame's longest draw, 5.5-6.2 ms** |

The likeliest mechanism is the combination rather than any one of them: an integer-format MRT write out
of a fragment shader with this much dynamic control flow gives ANGLE's D3D11 translation a shape it
handles badly, the draw's cost explodes, and the GPU watchdog (TDR) resets the driver — which presents
exactly as a browser hang rather than an error.

**Why this matters for [B1](blockers.md).** It reopens a route I closed too early. The blocker's option
list assumed no second attachment was possible at all; in fact the untested question is whether a
**narrower, non-integer** attachment on the same pass behaves differently — e.g. `rgba8unorm` carrying a
u8 tile-local id, which is 4 B/texel like the bake rather than 16. That is a genuinely different
experiment from the one that hung, not a retry of it.

Still the user's call ([B1](blockers.md)) because it is another live-fire test on the pass that has hung
twice — but the option set is wider than B1 states, and B1's "dead end" verdict applies only to the
integer path.

## I7 — The caster id alone cannot re-find a caster: positions are stored mod-16-tiles

**Blocks P3 and P4 as planned.** Both assume that knowing *which* caster occluded is enough to re-test
or refine it. It is not, because a caster's position is not absolute:

```glsl
vec2 resolvedTilePos(uint tile, uint unit, vec2 ref) {
  vec2 lm = ...;                      // 4+4 bits of tile, 4+4 of unit
  float period = 16.0 * UPT;          // 16 TILES
  vec2 d = lm - ref;
  d -= period * floor(d / period + 0.5);   // nearest wrap to ref
  return ref + d;
}
```

The record holds only the low 4 bits of the caster's tile coords. The walk supplies `ref` — the tile
whose caster bucket it was reading — and the position resolves to the nearest wrap of that. **`ref` must
be within ±8 tiles of the true caster or the position decodes to a phantom 16 tiles away.**

`casterOne` takes `ref` for exactly this reason. An incumbent recovered from the id map has no `ref`: the
bucket tile is what the walk knew and the texel did not store.

**Why substituting the receiver's own tile is not safe.** A caster must lie between the light and the
texel, so at reach 16 it can be ~16 tiles off — beyond the ±8 the wrap tolerates. And the failure is not
benign: a mis-decoded caster lands at a plausible position and may *occlude*, so the early-out would
accept a **false positive** and paint a phantom shadow. It is not the harmless "wasted test, fall through
to the walk" case the design assumed.

**Options for a successor:**

1. **Store the tile too.** Needs ~10 more bits per slot beyond the 15-bit id. There is no room in the
   128-bit texel at 8 slots, so this reopens the storage problem [F7](forks.md#f7) just closed — but now
   with a concrete bit count rather than a guess: 8 slots x ~25 bits = 200 bits, i.e. a second texel.
2. **Store a receiver-relative offset instead of the bucket tile** — dx,dy in tiles, signed. Same bit
   pressure, but the reference is free (the texel knows its own tile), and it could be clamped to a
   range where the wrap is unambiguous, accepting a miss beyond it.
3. **Widen the caster record's position field** so `resolvedTilePos` needs no `ref`. Touches the data
   texture layout, which `VARIABLES.md` owns.
4. **Drop P3/P4** and close the stream on P1+P2's banked wins.

**Nothing was shipped.** The P3 edit was reverted before loading — the flaw was found by reading
`resolvedTilePos` while wiring `casterOne`, not by a failure on screen. Tree clean at the P2 commit.

## I8 — The header px doubles the gather's fragment count, and at N=1 it carries a constant

Found 2026-07-31 while scoping the v6 layout, **before writing any of it**. Recorded because the
lighting strip will face the same arithmetic.

**A fragment writes 128 bits.** The v6 texel is 256 (header + ids). There are exactly three ways to
emit 256 bits per texel, and each has a cost the layout did not price:

| route | cost |
|---|---|
| MRT (two attachments) | hung Chrome twice ([I6](#i6)); never root-caused |
| 2× wide RT, gather rasterizes both px | **the walk runs twice per texel** — and the walk is ~87 % of the pass |
| a second derive pass reading the id RT | legal and cheap, but at N=1 the header is a pure function of the ids, so it carries no information |

**And today the header's payload is constant.** Every caster the walk considers is a billboard — the
loop skips anything else outright (`(slot >> 24) & 15u != SET_BILLBOARD_DATA`). So `u4 set` is **6 for
every occupied slot and 0 for every empty one**, which the id lane's sentinel already encodes. The
header only starts carrying information when a second caster *set* can win a slot — walls casting from
`definition_data`, say — which is exactly the direction the strip is likely to go.

**This does not invalidate F8.** F8 has two halves and only the second is affected: the parentless-
billboard reuse (self-positioning roots) is built, verified and independent — it needed no extra px at
all. It is the u20 half that wants a storage answer, and the answer is not "one more px" for free.

**For the rethink.** The real question the header was answering is *how wide is a caster reference*, and
u20 does not fit 8-per-texel. Two shapes close it without a second px, and both are the successor's
call, not mine: **6 slots × u20 = 120 bits** (one px, full reference, two fewer lights per tile), or a
**narrower reference** — a caster-local index into a per-window table rather than a global record id.
