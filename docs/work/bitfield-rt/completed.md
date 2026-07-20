# Completed — bitfield-rt

_Done + verified. Items move here from [`todo.md`](todo.md)._

---

## E1–E4 · Storage proof — DONE + verified in-browser 2026-07-20

At `?focus=100,50` + `/overlayRT lightmap-cold`: a clean **16×16 grid of 256 rects**, each one of **24
hues** (`bit = index mod 24`, so a per-row diagonal shift), **all 24 colours present**, no black / smeared
/ off-by-one rects. **A packed bitfield survives the world-space RT round-trip.** The config that worked:

- **RT** = a normal `SquareCache` **cold channel** (unorm RGBA8; `nearest` from the global
  `TextureStyle.defaultOptions`; linear, not sRGB) — so it rides the toroidal window / apron / reproject
  like every other composite for free.
- **Fill** = a per-square `fill` hook ([`ChannelSpec.fill`](../../../client/pixijs/src/game/viewport/SquareCache.ts))
  that clears the scratch to the packed colour at **A = 1** and blits — a raw framebuffer clear, no
  premultiply/gamma. **A held at 1** means premultiply is a no-op, so **no verbatim-write trick was needed
  at 24 bits** (guards [I-1](issues.md#i-1)/[I-5](issues.md#i-5) satisfied trivially).
- **Decode** = `/overlayRT` **OVERLAY_BITS** mode: float-mod bit extraction (see [I-8](issues.md#i-8)),
  HSV palette (hue = bit/24), additive.

**Key finding — [I-8](issues.md#i-8): Pixi's high-shader compiles GLSL ES 1.00, not ES 3.00.** `uint`/
bitwise is unavailable through the bit system; float-mod is the working path. This **reverses the
"ES 3.00 is free" premise** behind F14 / `rendering-platform.md`, now corrected.

**Remaining:** E5 (ping-pong read-modify-write proof — the accumulate path `shadow-cold` needs), E6
(graduate the proven config into `shadows`).
