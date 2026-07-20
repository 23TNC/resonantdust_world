# Work — shadow-tiered (screen-space realtime cast → world-space persistent bitfield)

_Opened 2026-07-20. Reworks the shadow system after `shadow-world` was found to **alias off-window
lights** (it cast shadows directly into the toroidal buffer via a raw `mod`, with no window-bounds check
— so a light outside the resident window stamped its shadow onto whatever square currently occupied its
mod-slot). Component: [`client/pixijs`](../../components/client/pixijs/). This is the design's
`shadow-hot → shadow-cold` split, done with correct ping-pong discipline — and it's window-correct for
the same reason the per-square composite bake is._

## Why — cast where it's window-bounded, store where it must live

The composites (albedo/normal/…) never alias because they bake **per resident rectangle**: the loop is
over in-window squares, so an off-window position simply never gets processed. `shadow-world` inverted
that (stamp a world position into the buffer by `mod`), which lost the window check. The fix is to cast
realtime shadows in the space that's window-bounded by construction — **screen space** (you only cast
what's on screen) — then copy that into the toroidal world buffer with a **4-rect wrap** (a rectangle
wraps cleanly; arbitrary geometry doesn't). Persistent shadows live in the world buffer so they stick to
the world; the just-updated lights are cast fresh in screen space for zero lag.

## The four RTs

- **`screen-shadow-a` / `screen-shadow-b`** — **screen-space** bitfields, this frame's **realtime**
  lights' shadows (freshly cast). Ping-pong so a frame can read last frame's while writing this frame's.
- **`shadow-a` / `shadow-b`** — **toroidal world-space** bitfields, the **persistent** shadows (every
  light that isn't in this frame's realtime set). Ping-pong. Same layout as the composites, so
  `/overlayRT shadow-a` shows them world-aligned.

`l-a` / `l-b` = the set of lights refreshed on an a-frame / b-frame (the "realtime" set that frame —
driven by which lights moved; can be a rotating subset to exercise both tiers).

## The pipeline (ping-pong: read one buffer, write the other — never both)

**On an a-frame** (read the `-b` buffers, write the `-a` buffers):

1. **Collect `l-a`** — the lights refreshed this frame.
2. **`shadow-a = (shadow-b + screen-shadow-b) without l-a`** — carry everything forward (the persistent
   shadows + last frame's realtime shadows, now baking in) and **remove `l-a`'s bits** (they'll be
   re-cast fresh, so their *old* positions must be cleared — this is what kills the ghost when a light
   moves). See the bit algebra below for how "+ without" is done without a feedback loop.
3. **`screen-shadow-a = the shadows cast by l-a this frame`**, in **screen space**.
4. **Display `shadow-a` OR `screen-shadow-a`** — decode both bitfields to colours (world-space sample of
   `shadow-a`, screen-space sample of `screen-shadow-a`) and OR them.

**On a b-frame**, swap every `-a`↔`-b`.

Net effect: `shadow-*` accumulates all-but-the-current-realtime lights in world space; the realtime set
is always freshly cast in screen space and shown with zero lag; each frame hands last frame's realtime
set into the persistent store.

## The bit algebra (the two tricks)

Each light owns one bit; a bit is set by exactly **one** source. Given that **disjointness**:

- **ADD is OR, done with additive blend.** `b1010 + b0101 = b1111` — because the bits don't collide,
  a fixed-function **additive blend** (`src + dst`) equals OR. Crucially, additive blend reads the
  destination through the *blend unit*, **not** a texture fetch in the shader — so adding
  `screen-shadow-b` into `shadow-a` is **not** a feedback loop (that rule only forbids *sampling* the
  bound target). The disjointness invariant is load-bearing: if the same bit were set in both operands,
  `0x02 + 0x02 = 0x04` would carry into a *different light's* bit. Never let two sources share a bit at a
  pixel.
- **REMOVE can't subtract — clear per bit.** To drop `l-a`'s bits: a shader pass reads the source buffer
  and, for each light `i` in `l-a`, does `n -= (bit i set ? 2^i : 0)` (float-mod, ES 1.00 — the clear-bit
  op the combine already uses). This is the ping-pong read (`shadow-b`) → write (`shadow-a`) pass; source
  ≠ destination, so it's legal. `+ screen-shadow-b` then happens as the additive-blend step onto the
  just-written `shadow-a`.

So step 2 is really: **(shader) `shadow-a = shadow-b` with `l-a` cleared** → **(4× additive blend)** the
wrapped rects of `screen-shadow-b` onto `shadow-a`.

## Screen → world: the 4-rect copy

`screen-shadow-*` is screen space; `shadow-*` is the toroidal buffer. The screen rectangle maps into the
buffer at the window's position and **wraps into up to 4 rectangles** (H seam × V seam). So the merge is
**4 additive-blend blits** — the same wrap the composites' window uses, and window-bounded because the
screen only contains on-screen (in-window) shadows. (This needs the window origin, `winCol`/`winRow`,
which `SquareCache.bufferMapping()` must now expose — the missing piece that sank `shadow-world`.)

## Alignment with the durable design

This *is* [`intent/tiered-lighting.md`](../../components/client/pixijs/intent/tiered-lighting.md)'s
`shadow-hot` (screen, fresh) → `shadow-cold` (world, bitfield) with the round-robin being the `l-a`/`l-b`
rotation — re-derived from first principles with the correct read≠write ping-pong. If it proves out, it
graduates straight into the real [`shadows`](../shadows/README.md) engine.

## State

Phased in [`todo.md`](todo.md); decisions in [`forks.md`](forks.md); gotchas + the feedback in
[`issues.md`](issues.md). Supersedes `shadow-world`'s direct-world-cast (the aliasing bug). All bitfields
unorm RGBA8, A=1, float-mod (ES 1.00); **alpha never carries data**.
