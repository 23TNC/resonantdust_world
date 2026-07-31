# Forks — binary occlusion + stored caster ids

_Decision points, options, which we chose and why._

## F1 — Is penumbra dead, or broken? Decide binary anyway {#f1}

The user: _"Penumbra didn't work. You can say it did all you want... but at the end of the day this
shadow has no penumbra."_ A zoom-1 screenshot of a conifer shadow shows a hard, stepped edge.

- (a) Fix penumbra first, then decide.
- **(b) Go binary now; histogram the coverage in P0 for the record, non-blocking.**
- (c) Go binary and don't look.

**Chosen: (b).** The user has seen the screen and I had not — I claimed penumbra was "live" from reading
`radius 0.35` and seeing the `frac` code path exist, which is asserting from constants instead of from
the render. That was the wrong basis and the screenshot settles it.

But the gap is too large to leave unrecorded. Geometry predicts `11.2 u × 26.2/(40−26.2)` ≈ **21 texels**
of gradient, and a histogram earlier in the session (at radius 2.0) found **61 % of shadowed texels
partial**. Something between "computed" and "displayed" is losing it. Five minutes of histogram tells a
successor whether this stream deleted working code or dead code — which is the difference between a
trade and a cleanup.

(a) inverts the user's explicit priority. (c) throws away the only cheap chance to know.

**If the histogram shows a live spread**, that is an issue to file, not a reason to stop: binary is still
the design, and it now has a known cost rather than an assumed-zero one.

## F2 — Keep 16 lights, or drop to 8? {#f2}

The user first proposed 8 (one presence set), then: _"Fine we can keep 16, it just doubles the number of
px we need to hold per slot and that's already really cheap."_

- (a) 8 lights, one id px per shadow texel (8 × `u16` = 128 bits), ~2 MiB/class.
- **(b) 16 lights, two id px per shadow texel (16 × `u16` = 256 bits), ~4 MiB/class.**

**Chosen: (b).** 2 MiB against 405 MiB resident is not a number worth trading a capability for. The
design is explicitly _"dense AUTHORED point lights, not a sun"_, and halving the per-tile ceiling to buy
2 MiB would be paying in the scarce currency to save the abundant one.

16 is also not an arbitrary ceiling — it is `128 bits / 8 bits per slot` exactly. Dropping to 8 would
leave the shadow texel half empty while a separate texture carried the ids.

## F3 — Binary frees 7 bits per slot. Use them for the id? {#f3}

Once coverage is 1 bit, each slot's byte has 7 spare bits — 112 spare bits per shadow texel.

- (a) Ignore them; put ids in their own texture ([F2](#f2)).
- (b) Pack a `u7` id into the freed bits: no second texture at all.
- **(c) Start with (a); evaluate (b) in P5 once the id map's real cost is measured.**

**Chosen: (c).** (b) is tempting and might be free, but `u7` caps caster ids at **128 per class-window**,
and `BILLBOARD_SLOTS = 8` per tile over a 32×16 slot grid means the live caster set can exceed that
easily. Truncating an id silently points the refine at the wrong prim — a failure that renders as a
plausible-but-wrong shadow rather than an error.

So: build it correct with `u16` first, measure, and only then ask whether a narrower id is safe. The
question is "how many distinct casters can be live in one window", and that is a measurement, not a
guess.

## F4 — One id per texel leaves the LIT side of the edge coarse {#f4}

The user's stated sacrifice is *within* a slot: one prim may not occlude all of it, fixed by raising N.
There is a companion gap that raising N does **not** fix.

**The refine can only remove shadow, never add it.** A coarse texel whose centre is lit stores no id, so
its fine texels stay lit — even where the true shadow boundary passes through them. The result is a sharp
edge on the shadowed side and a coarse step on the lit side: half a fix.

- (a) Accept; the shadowed side is the visible one.
- (b) Dilate the id at write time — a texel stores an id if it or a 4-neighbour is occluded.
- **(c) Read the 2×2 id neighbourhood at refine time and test whichever are non-empty.**

**Chosen: (c).** The fine pass **already fetches a 2×2 coarse neighbourhood** when `uShadowFilter` is on,
so the addressing exists and the taps are cache-resident. Ids in a neighbourhood are usually the same
prim, so dedupe makes the common case one test. It delivers N≈4 exactly where N matters — at the edge —
for **no extra memory**, which is strictly better than F2-style N-per-slot.

(b) costs write-time work and blurs which texel owns which caster. (a) leaves a coarse step on the side
of the edge the eye follows against bright ground.

## F5 — Does this stream turn the differential on? {#f5}

`max` → `any` makes the stored value **order-independent**, which is precisely what `DIFFERENTIAL_WIRED`
requires: a light's old contribution must be exactly reproducible to be subtracted. And
`coldShadowPrevRT` / `hotShadowPrevRT` are already allocated, never written, never read — 4 MiB idling.

- (a) Wire it in this stream while the invariant is fresh.
- **(b) Record it in [`issues.md`](issues.md); build it separately.**

**Chosen: (b).** This stream already changes the shadow encoding, the caster combine rule, the storage
layout and the lighting pass. Adding the differential would put a second independent variable into every
identity check, and the headline measurement — lights at reach 16 within 8 ms — would no longer
attribute cleanly.

It is a real unlock and it should not be lost, which is why P6 writes it down with the flag name and the
idle buffers named explicitly.
