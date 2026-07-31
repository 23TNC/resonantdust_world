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

## F7 — REVERSES F2: 8 lights per tile, and the texel becomes the id map {#f7}

**User, 2026-07-31, after three failed approaches from me.** F2 chose to keep 16 lights and find the ids
a new home. That was the wrong half to hold fixed.

At **8** slots the arithmetic closes with nothing left over:

```
8 slots x u16 = 128 bits = ONE uvec4 = the shadow texel we already have
```

No second attachment (which hung twice), no 2x-wide RT (my elaboration, now unnecessary), no packing a
short id into spare bits (impossible — an 8-bit id cannot index a u16 billboardIdx). The texel stops
storing a value and stores the **caster id**, with a sentinel for "not occluded" — so occlusion is
implied, not stored, exactly as the user said before I understood it.

**Why 8 is not the constraint I claimed.** F2 kept 16 partly on the grounds that halving would starve a
scene of "dense AUTHORED point lights". That reasoning was wrong: `PRES_SLOTS` is **per tile**, and the
sets are independent — tile A can hold lights 1-8 while tile B holds 9-16. The cap is on *overlap at one
tile*, not on how many lights exist. Eight lights reaching a single tile is already a lot, and the user's
position is explicit: start at 8, double back to 16 only if a real scene hits it.

**Bonus, unasked for:** it halves the gather's inner loop from 16 iterations to 8. That is a second
speed win stacked on P1's, in the pass that is 87% of the frame cost.

**Layout.** `u16` per slot = `onBillboard << 15 | billboardIdx (15 bits)`, sentinel `0x7fff`/`0xffff` for
no caster — which also rehomes the on-billboard flag that was being stored 16 times for one bit of
per-texel information.

**Supersedes:** F2 (16 slots), F3 (spare-bit packing — moot, no spare bits and none needed),
[B1](blockers.md) (dissolved: the route needing the user's risk tolerance is gone).

## F8 — A caster reference is u20, not u16: header px + N id px {#f8}

**User, 2026-07-31.** Two gaps in F7's layout, both mine, both found by the user:

**1. Billboards without a parent were never handled.** `billboard_data` R is
`u16 parent_id (16-31) | u8 resolved_tile (8-15) | u8 resolved_unit (0-7)`. For a parentless billboard
`parent_id` carries nothing, so it is reused: **`u8 region | u8 zone`**, the same addressing `prim_data`
uses, gated by a new **`u1 child`** bit taken from A's `u14 reserved (0-13)`. Child set -> the field is a
parent id as today; child clear -> it is region|zone.

That is what kills [I7](issues.md): `resolvedTilePos` needed a `ref` only because the record lacked the
HIGH bits of position. region|zone supplies exactly those, so a caster record becomes
**self-positioning** and no reference tile is needed at all. My "store a dx,dy offset" patch is
unnecessary.

**2. The reference is u20, not u16.** The data texture is *16 u16-addressable SETS*
(`set = linear >> 16`), so identifying a record takes `u4 set | u16 in-set id` = **20 bits**. 8 casters x
u20 = 160 bits, which never fitted the 128-bit texel. F7's arithmetic was wrong the moment it assumed
u16 was a whole reference.

**The layout, per the user:**

```
px 0  HEADER   4 channels x u32; each channel serves 2 lights
               => 16 bits per light = 4 x u4 set  => supports N = 1..4 casters/light
px 1  IDS      8 lights x u16 in-set id                        (N = 1)
px 2..4        further id px, one per extra caster             (N = 2..4)
```

So the shadow texel becomes **2 px at N=1**, growing to 5 px at N=4 — and N is now a real dial rather
than a rewrite, which is what the user asked for from the start ("expand to N prims... increases the px
count per slot").

**Supersedes** F7's single-px claim and the id-width half of [I7](issues.md)/[B2](blockers.md). The
storage route is the 2x-wide RT (single attachment, no MRT, never hung) that F7 dismissed as
unnecessary — it is necessary after all, for a reason F7 did not know.
