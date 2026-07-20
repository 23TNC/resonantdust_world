# Forks — bitfield-rt

_Decision points + options + which we chose + why. Chronological, all 2026-07-20._

---

## F1 · Integer `RGBA8UI` RT (preferred) vs unorm RGBA8 + verbatim write — 2026-07-20

**Chose the integer `RGBA8UI` target.** It removes premultiply, blending, and sRGB from the picture by
construction ([I-1](issues.md#i-1), [I-3](issues.md#i-3)), and gives real `uint` bitwise + `texelFetch` on
read — exactly the ES 3.00 win ([F14 in shadows](../shadows/forks.md#f14)). **Fallback:** if wiring an
integer RT through Pixi v8's RenderTexture proves fiddly, fall back to a unorm RGBA8 with a **verbatim
non-premultiply** write (the nuked build's "linchpin") + `uint(v*255+0.5)` decode. The experiment's job
includes finding out which is clean in this stack — that's a real thing to learn here.

## F2 · Fill via a dedicated pass, not the per-prim bake — 2026-07-20

`SquareCache`'s bake resolves **per-prim**, but this experiment writes a **per-rect constant** (a bitfield
keyed to the rect's index), independent of any prim. **Chose** a dedicated **fill pass** over the RT that
computes the bitfield from the rect/world coordinate, rather than shoehorning it into the per-prim channel
resolve. Simplest, and it tests the RT write path directly — which is the point.

## F3 · One-hot per rect, `bit = index mod 24` — 2026-07-20

Each rect sets **exactly one** bit; rect index `0..255` → bit `index mod 24`. This makes the proof
unambiguous: every rect maps to a known, single colour, so any wrong colour is a clear, localised
failure. (Multi-bit / additive overlap — what `shadows` ultimately needs — is the optional E5 stretch,
kept out of the base case so the one-hot proof stays clean.)

## F4 · 24-colour palette, indexed by set-bit — 2026-07-20

The decode maps bit index `0..23` → a **fixed 24-colour palette** (distinct, roughly evenly spread in
hue so adjacent bits read differently). One-hot rects show a pure palette colour; if E5's multi-bit
stretch runs, the decode **sums** the set bits' colours (additive) so overlaps are visible — the same
decode `shadows`' `/overlayRT shadow-cold` will use.
