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

## I-6 · Feedback loop — add via blend, remove/cast via ping-pong — open

Never **sample** (texture-fetch) the RT bound as the current target — that's the UB feedback loop. But
fixed-function **blend** reading the target is fine (it's not a fetch), so the additive-blend ADD onto
`shadow-a` is legal. The remove pass avoids it by ping-pong (read `-b`, write `-a`). Keep this distinction
crisp: blend = OK, in-shader sample of the target = not OK.

## I-7 · Cost / scaling note — open

Per frame: 1 screen cast (realtime set) + 1 remove pass (full buffer) + 4 additive blits (screen→world) +
1 display. Four persistent RTs (2 screen + 2 world). Fine at 5 lights; at 24 it's the same pass count
(bits widen to RGB, cast set grows). Log it if a real budget bites.
