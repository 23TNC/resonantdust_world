# Work — bitfield-rt (prove a bitfield survives a render target)

_Opened 2026-07-20. A **de-risking experiment**, not production. Before rebuilding the shadow engine
([`../shadows/`](../shadows/README.md)), prove the one mechanism that has failed repeatedly: **writing a
packed bitfield into a render target, storing it, and reading it back intact.** Strip away lights,
casters, projection, screen→world copy, round-robin — everything — and test *only* the RT round-trip.
Component: [`client/pixijs`](../../components/client/pixijs/). Depends on the WebGL2 / GLSL ES 3.00 floor
([`design/rendering-platform.md`](../../components/client/pixijs/design/rendering-platform.md))._

## Why — one mechanism, isolated, so a failure has one cause

The nuked lighting attempt hit "a ton of issues" storing occlusion in an RT — premultiply zeroing bytes,
bilinear filtering garbling bits, format/sRGB surprises, coordinate mismatches. Those all got tangled
with the shadow projection, so a wrong pixel could be *any* of a dozen causes. This experiment removes
every variable but the RT itself: fill a bitfield by hand, read it back, colour it. If the colours are
right, **the storage layer is proven** and `shadows` can be built on it with confidence. If they're
wrong, we've found the bug in the smallest possible harness. **Prove the theory before trying again.**

## The experiment

1. **Spin up a world-space bitfield RT** — named `lightmap-cold` (see the naming note below), in the cold
   `SquareCache`'s **toroidal world-space** layout so it pans + scales with the other composites, as an
   ES 3.00 **integer** target (`RGBA8UI`), **nearest**, **no blend**, **linear** (not sRGB).
2. **Populate each rect with a one-hot bitfield.** For the **16×16 zone at focus `(100, 50)`** — 256
   tile-rects — write into each rect a value with **exactly one bit set**: rect index `i ∈ 0..255` →
   bit `i mod 24`, packed into **RGB** (bits 0–7 → R, 8–15 → G, 16–23 → B; A unused). Rects outside the
   zone stay 0.
3. **Decode each rect to one of 24 colours.** `/overlayRT lightmap-cold` reads the RT (`texelFetch`),
   finds the set bit, and paints the rect that bit's unique colour — 24 distinct colours across the 256
   rects (bits cycle: 0–15 appear 11×, 16–23 appear 10×).

That's the whole thing: **write bits → store → read bits → colour**. 256 rects, 24 colours, no shadows.

## Pass / fail — and what each failure diagnoses

**PASS:** at `?focus=100,50`, overlay on → a clean **16×16 grid of 256 solid-coloured rects**, each rect
the colour of its assigned bit, **all 24 colours present**, colours **stable as you pan** (world-space RT
→ rects keep their colour while scrolling) and **clean under zoom** (nearest scaling, no bleed).

**FAIL modes — each points at one specific bug** (this is the payoff of the isolation):

| symptom | cause |
|---|---|
| rects are **black / zero** | premultiply killed the byte (A path) — the classic linchpin bug |
| **smeared / wrong colours at rect edges** | bilinear filtering interpolating the byte → garbage bits (need `nearest`) |
| **off-by-one bit** (rect shows neighbour's colour) | float rounding on decode, or an **sRGB** RT gamma-mangling the byte |
| colours **shift when you pan / zoom** | world→buffer coordinate or reproject mapping is wrong |
| **all one colour** / no variation | the per-rect write isn't landing (fill pass addressing the rect wrong) |

## Relationship to `shadows`

This proves exactly the storage layer `shadows`' **`shadow-cold`** depends on — a world-space,
nearest, no-blend, integer bitfield RT read back per-texel with real `uint` bitwise. Once green, `shadows`
P1/P3/P6 (create `shadow-cold`, pack into it, decode it) inherit a proven mechanism; the only thing left
there is *generating* the bits (screen-space casting + the 4-copy), not *storing* them. This experiment
is the gate before `shadows` P0.

## Naming note

The RT is called **`lightmap-cold`** as requested. Be aware this differs from the tiered design's
`lightmap-cold` (a *baked RGB light sum*, not a bitfield). Here it's a raw bitfield purely to prove the
mechanism; the mechanism is what the real **`shadow-cold`** needs. When graduating into `shadows`, the
proven RT becomes `shadow-cold` — the experiment doesn't depend on the name, only the round-trip.

## State

Phased in [`todo.md`](todo.md); decisions in [`forks.md`](forks.md); the known-gotcha guards (the "ton of
issues" we're proving against) in [`issues.md`](issues.md). Throwaway by intent — once it proves out,
its findings graduate into `shadows` and this folder can be archived.
