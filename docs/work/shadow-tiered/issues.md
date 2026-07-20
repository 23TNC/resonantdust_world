# Issues — shadow-tiered

_Gotchas + the design feedback, seeded on open 2026-07-20. The pipeline is sound; these are the sharp
edges to respect when building it._

---

## I-1 · Additive-blend ADD is exact-OR ONLY for disjoint bits — open

The merge uses fixed-function additive blend (`src + dst`), which equals OR **only if no two operands set
the same bit at a pixel**. If they do, the byte carries: `0x02 + 0x02 = 0x04` — light 1's bit becomes
light 2's. **Guard:** each light owns one bit set by exactly one source, and the removal pass (I-2) clears
the realtime set from the persistent *before* re-adding it. Keep `l-a`/`l-b` disjoint (or clear the
overlap) so `screen-shadow-b` and `shadow-a` never share a bit at the merge. On unorm RGBA8 the byte-sum
is exact for totals ≤ 255 (≤ 8 bits); keep **A = 1**.

## I-2 · Removing bits — clear per bit, never subtract — open

`shadow − someLights` isn't a subtraction (borrow crosses bit boundaries). Clear each removed light's bit:
`n -= (bit i set ? 2^i : 0)` per light `i` (float-mod, ES 1.00). This is a shader RMW: read `shadow-b`,
clear, write `shadow-a` (ping-pong, source ≠ destination — legal).

## I-3 · Don't make every light realtime every frame — open (feedback)

If `l-a`/`l-b` = all lights each frame, `shadow-*` is cleared of everything every frame → the world tier
holds nothing and the whole thing degenerates to screen-space (defeating both the persistence *and* the
alias fix). The realtime set must be the **changed** lights (or a rotating subset). For the 5-light
experiment: put a moved light into `l` on its move frame; optionally rotate a subset so both tiers stay
busy ([F5](forks.md#f5)).

## I-4 · The display samples two different spaces — open

`shadow-*` is world-space (sample at the display-geometry UV); `screen-shadow-*` is screen-space (sample
at `gl_FragCoord` / a screen UV). The decode ORs both at the same on-screen pixel. Straightforward, but
the display shader needs both a world-UV sampler and a screen-coord sampler — don't assume one space.

## I-5 · `winCol`/`winRow` must be exposed (the shadow-world gap) — open

The screen→world 4-rect copy needs the resident window's world origin to know *where* the screen maps in
the toroidal buffer. `bufferMapping()` currently returns only sizes/counts; add `winCol`/`winRow`. This is
the exact information `shadow-world` lacked, which let it alias.

## I-6 · Feedback loop — the rule is narrow: no sampling your own target in one draw — open

The ONLY thing blocked is **sampling (texture-fetch) the RT bound as the current draw's target** (source
== destination in one pass). That's it — the temporal pattern across passes is irrelevant:
- **Blocked:** `shadow-a = f(sample(shadow-a), …)` in one draw. (Why `shadow = (shadow+sb)−l` in place
  needs ping-pong.)
- **Fine:** `shadow-b = f(sample(screen-shadow-a), sample(shadow-a), …)` — any number of sources, none of
  them the target. "write X → later read X → write Y" (Y≠X) is fine; it's not read-modify-write of X.

So the merge can be done EITHER as a single shader pass (read `shadow-b` + `screen-shadow-b` remapped,
write `shadow-a` — legal, since `shadow-a` isn't sampled) OR as a clear pass + additive-blend blits (blend
reads the target through the blend unit, not a fetch — also legal). Both are safe; the 4-rect additive
blit is the simpler geometry ([F3](forks.md#f3)).

## I-8 · Deferred merge must use the CAST frame's window, not the current one — open

`screen-shadow-b` was cast on the previous (b) frame at *that* frame's screen/pan; its pixels correspond
to fixed **world** ground positions. Merging it into `shadow-a` on this (a) frame, the screen→world
placement (the 4-rect wrap) must use the **window origin / pan from the frame it was cast**, not the
current frame's — otherwise a pan between cast and merge slides the shadows off their world positions.
**Guard:** stash `winCol`/`winRow` (+ pan/zoom) at cast time and use them at merge time. (The 3-RT
same-frame merge avoids this by never deferring — the tradeoff we accepted in [F2](forks.md#f2).)

## I-7 · Cost / scaling note — open

Per frame: 1 screen cast (realtime set) + 1 remove pass (full buffer) + 4 additive blits (screen→world) +
1 display. Four persistent RTs (2 screen + 2 world). Fine at 5 lights; at 24 it's the same pass count
(bits widen to RGB, cast set grows). Log it if a real budget bites.
