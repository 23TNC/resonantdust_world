# Forks — shadows

_Decision points + options + which we chose + why. Chronological. Most of these are the user's
call for this foundation slice (recorded so the rationale survives); F6/F7 are ones resolved here._

---

## F1 · `shadow-cold` bit storage: RED byte — 2026-07-19

**Options:** (a) a full 32-bit bitfield across the RGBA texel (the target design); (b) a single **RED
byte** (8 bits). **Chose (b)** for the foundation — 6 lights fit in one byte, and packing into one
channel keeps the combine shader trivial (one byte to OR into) while still proving the bit mechanism.
Widening to 32 (all 4 bytes) is [D-1](deviations.md#d-1), a later phase.

## F2 · `shadow-hot` lanes: RGB, not A — 2026-07-19

**Options:** (a) RGBA = 4 lanes; (b) **RGB = 3 lanes, A free**. **Chose (b).** The alpha channel is the
premultiply/opacity lane on every Sprite/blit path — storing shadow data there means a premultiply pass
silently multiplies it into RGB and zeroes it when "transparent". Keeping data in RGB and A untouched
sidesteps that entirely. 3 lanes ⇒ batches of 3 ⇒ 2 batches for 6. (The target design reclaims the 4th
lane with the `uChannel` trick — deferred, [D-2](deviations.md#d-2).)

## F3 · Casters: pure billboards, no textures/outlines — 2026-07-19

**Options:** (a) the textured earcut silhouette (`outline` sidecar) with per-triangle UV alpha, per
[`design/shadows.md`](../../components/client/pixijs/design/shadows.md); (b) the **solid billboard quad**
projected to the ground. **Chose (b)** for the foundation. The projection math + RT plumbing + packing are
the parts being proven here; the silhouette/UV/alpha refinement is a shader-detail layer that drops on top
of a working pipeline without changing it. Logged as [D-3](deviations.md#d-3).

## F4 · Batching: 6 lights as 2 × 3 — 2026-07-19

Fixed by F1 (byte) + F2 (RGB): 3 lanes per stage ⇒ 2 stages for 6 bits. General rule for the widen: with a
`shadow-hot` of `L` lanes and a bitfield of `B` bits, fill in `ceil(B/L)` batches.

## F5 · Overlay: 6 unique colours, additive overlap — 2026-07-19

Each of the 6 bits gets a distinct colour; a fragment sums the colours of its set bits so **overlapping
shadows combine** (e.g. two lights → a blended hue), and a fragment with no bits set is transparent. This
makes the bitfield legible at a glance — you can see which light cast which shadow and where they overlap.
(Suggested palette: the 6 primaries/secondaries R, G, B, Y, M, C, so pairwise sums stay distinguishable.)

## F6 · Where the 6 cold lights come from — RESOLVED 2026-07-19

**Open:** `LightRig` (and its `/coldlight`) were deleted; the foundation needs a light source but not the
rig. **Options:** (a) hard-code 6 lights at boot; (b) a **debug chat command** `/coldlights [x y]` (+
`?coldlights` URL) that seeds a ring of 6 around a tile, default (100, 50). **Chose (b)** — matches the
existing debug-command pattern (`/focus`, the removed `/coldlight`), keeps the lights inspectable/movable
during bring-up, and doesn't bake test data into boot. The lights are a plain array on the viewport, not a
revived rig.

## F7 · `shadow-cold` = a world-space cold channel, `shadow-hot` = per-rect scratch — RESOLVED 2026-07-19

**Open:** are these persisted `SquareCache` composites or screen-space RTs? **Chose:** `shadow-cold` is a
**cold-tier composite** (world-space, per-rect, dirty-baked) — so `/overlayRT` can sample it through the
same world-aligned display geometry as the other channels, and so it re-bakes only when a rect's casters
or in-range lights change (the whole point of "cold"). `shadow-hot` is a **per-rect scratch RT** (one slot,
reused) — it's staging consumed inside the same bake, never displayed, so it needn't persist per rect. This
matches the target design (`shadow-cold` a cold composite; the scatter maps transient).
