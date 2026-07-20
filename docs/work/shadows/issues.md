# Issues — shadows

_Problems + candidate solutions + which we chose + why. Chronological. Seeded on open with the
technical gotchas this plan already anticipates (from the nuked build's hard-won lessons), so they're
resolved deliberately rather than re-discovered. Move to "resolved" with a date as they land._

---

## I-1 · Bit set/test on GLSL ES 1.00 (no integer ops) — open, 2026-07-19

The most-touched shaders are ES-1.00 (no `uint`, no bit operators). Both the **pack** (set bit `i`) and
the **overlay** (test bit `i`) must be float math. `rgba8` stores `n/255` exactly, so:
- **test:** `mod(floor(red*255 / exp2(float(i))), 2.0)` is exact for bits 0..7.
- **set (pack):** each batch writes *distinct* bits into a byte that already has the other batch's bits,
  so `red' = prevRed + present · (exp2(i)/255)` is an OR (never a double-count). This is proven-safe from
  the nuked `warmCombine`; keep it.

## I-2 · Premultiply corrupts a bitfield byte — sidestepped at 6 bits, 2026-07-19

A Sprite/tint batch **premultiplies** `RGB × A`, which zeroes data bytes where A=0. The nuked build hit
this and needed a **verbatim non-premultiply Mesh blit** ("bitfield linchpin"). This stream sidesteps it:
`shadow-cold` **A is held at 1**, so RGB×1 is a no-op and any path is safe. **Caveat for the widen**
([D-1](deviations.md#d-1)): once A becomes a 32nd data lane, the verbatim Mesh blit must return.

## I-3 · `shadow-hot` channel writes without an add-overflow — open, 2026-07-19

Multiple casters for the **same** light must **union** into that light's channel, not sum past 1. Use a
per-light channel-write shader (`outColor = uChannel`, a 1 in one lane) with **`max` blend** — overlapping
caster quads clamp at 1. `add` would overflow; the tiered-lighting anti-goals call this out explicitly.

## I-4 · `shadow-cold` must be world-aligned for the overlay — resolved (F7), 2026-07-19

`/overlayRT` samples composites through the **world-aligned display geometry**. A screen-space shadow RT
wouldn't line up. Resolved by [F7](forks.md#f7): `shadow-cold` is a **world-space cold composite** baked
per-rect, so it shares the exact register of `albedo-cold` et al. `shadow-hot` (scratch, never displayed)
has no such constraint.

## I-5 · Re-bake on light change — open, 2026-07-19

`shadow-cold` is cold (dirty-baked), but the 6 debug lights are dynamic during bring-up (`/coldlights`
moves them). A baked rect isn't geometry-dirty, so moving a light won't re-bake its shadow on its own.
Mirror the nuked build's fix: when the cold-light set changes, **invalidate all rects** so the bitfield
re-bakes. (Fine at 6 debug lights; real cold lights rarely move.)
