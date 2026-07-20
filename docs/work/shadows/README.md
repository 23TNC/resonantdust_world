# Work — shadows (the screen-hot → world-cold bitfield foundation)

_Opened 2026-07-19. The clean restart after the tiered-lighting port was nuked (the reverted
`lighting` stream, archived out-of-repo). This stream builds the **shadow RT + bit-packing pipeline**
from scratch, at the smallest scale that exercises every moving part, so the full design can grow on top
of a working, understood core. Component: [`client/pixijs`](../../components/client/pixijs/). Builds
toward [`intent/tiered-lighting.md`](../../components/client/pixijs/intent/tiered-lighting.md) (the
shared bitfield shadow engine) and
[`design/shadows.md`](../../components/client/pixijs/design/shadows.md) (the billboard-quad projection)._

## Why — stop fighting the rects and world-space

The nuked attempt died **fighting the rects and world-space**: it baked shadows **per-rect in
world-space**, which forced two problems that never resolved cleanly —

- **partial shadows** — a shadow crossing a rect boundary had to be split and drawn into each rect, and
- **which-rect-when-a-light-moves** — moving a light meant recomputing which rects its shadow now touches
  and re-dirtying them.

This stream **sidesteps both** by generating shadows in the space they're easy in and translating once:

> **`shadow-hot` is SCREEN space. `shadow-cold` is WORLD space. One copy bridges them.**

Shadows are cast in **screen space** into `shadow-hot` — the whole viewport at once, so there are no rect
boundaries to split across and no per-rect bookkeeping. `shadow-hot` is thrown away and **regenerated
every frame**, so it never goes stale and never needs reprojection. Then its screen-space result is
**copied into the world-space `shadow-cold`** in one step (see the toroidal copy below). That's it: cast
where it's easy (screen), store where it must live (world), bridge with a copy.

## The two RTs

- **`shadow-hot`** — a **screen-space** RGBA8 RT, viewport-sized, **regenerated every frame**.
  **RGB = three shadow lanes**, one light each; **A unused** (A never survives premultiply — see
  [F2](forks.md#f2)). Because it's rebuilt each frame at the current zoom, it has **no staleness and no
  scaling problem** by construction.
- **`shadow-cold`** — a **world-space** bitfield RT, in the **same toroidal layout the other G-buffer RTs
  use** (so it pans + scales with zoom exactly like `albedo-cold` et al.). **It does NOT hold per-rect
  geometry like the other RTs — it holds LIGHTS**: each **bit** of a pixel = "light `i` shadows this world
  point". **This iteration: 6 lights in the RED byte** (unorm RGBA8, A=1 — premultiply is a no-op).
  **Goal: 24 lights in RGB** — the **alpha channel is never used for data** (premultiply-error-prone;
  [F11](forks.md#f11), settled). Beyond 24 → more render targets, gated behind raw ES 3.00 for MRT
  (deferred).

## The screen → world translation (the 4-copy)

`shadow-cold` lives in the SquareCache's **toroidal** world-space buffer — the viewport window wraps
around the buffer's seams. So the screen-space `shadow-hot` rectangle lands as **up to 4 rectangles** in
the world-space layout (it can straddle the horizontal seam, the vertical seam, or both → 4 quadrants).
The translation is therefore **4 copy commands**, each blitting one wrapped quadrant of `shadow-hot`
into `shadow-cold` at its world position. This is the *same* toroidal-wrap the cache already does for its
window; we reuse it.

The copy is where the **bit-pack** happens: each copy reads `shadow-hot`'s RGB (this frame's 3 lights)
plus the existing `shadow-cold`, sets those 3 lights' **bits**, and writes back — preserving the other
bits. So "copy" = "pack-and-copy, 4× for the wrap".

**This is the whole point:** casting in screen space kills the partial-shadow split, and the single
copy-to-world kills the which-rect-when-a-light-moves bookkeeping — a moved light just re-casts into the
fresh screen-space `shadow-hot` next frame and copies in.

## The round-robin — 3 hot lights/frame → 24 cold in ~8 frames

`shadow-hot` holds **3 lights/frame**. Each frame we cast the **next 3** lights, copy them into their 3
bits of `shadow-cold`, and advance. With the 24-light goal that's **8 frames to refresh all 24** (~130ms
at 60fps) — a light's shadow is at most that stale. This is the design's warm-tier round-robin, and it's
why **"cold" here is provisional: we'll swap cold → warm later** ([D-6](deviations.md#d-6)). This
iteration proves it with **6 lights** (2 frames to fill the RED byte).

## Zoom — cold scales with the other RTs; hot has no scale issue

On zoom, `shadow-cold` **scales the same as the other G-buffer RTs** (it shares their window/slot
geometry). Because `shadow-hot` is **regenerated every frame at the current zoom**, we're always casting
shadows at the right scale and copying them into the freshly-scaled `shadow-cold` — **no shadow-specific
scaling artifacts**. (A bitfield can't be *bilinearly* resampled — that garbles the bits — so the
zoom-reproject of `shadow-cold` is **nearest**, and the round-robin refills any not-yet-refreshed bits
over the next few frames; see [I-1](issues.md#i-1).)

## Casters — pure billboards

A caster is the standing prim's **billboard quad** (W×H box, tilted by the ground angle), projected
radially from the light to the ground per
[`design/shadows.md` §Projection](../../components/client/pixijs/design/shadows.md) — drawn as a **solid
2-triangle quad** in screen space. **No texture sampling, no alpha mask, no `outline` silhouette** — those
are the next layer, deliberately out of scope ([D-3](deviations.md#d-3)).

## The overlay proves it

`/overlayRT shadow-cold` gets a new decode: read the pixel's bits and paint **each set bit a unique
colour**. This iteration: **6 colours for the 6 RED bits**; at the goal, 24 colours across RGB.
**Overlapping shadows combine colours** (additive), so a spot shadowed by lights 0 and 1 shows colour 0 +
colour 1. This is the only "display" in the stream — there's no lit render yet, the overlay *is* the
verification surface.

## How this maps onto the target design (and where it intentionally shrinks / diverges)

Deliberate, pre-logged in [`deviations.md`](deviations.md):

| target design | this foundation | later |
|---|---|---|
| cold **baked per-rect in world-space** (on dirty) + separate warm round-robin | **one** bitfield: cast **screen-space** every frame, copied into **world-space** `shadow-cold`, round-robin | D-5 → split back into a true baked-cold tier + warm tier |
| `shadow-cold` = **32-bit** across RGBA | **RED byte** (6 bits) → **RGB = 24** (A never used for data) | D-1 → widen to RGB; >24 needs more RTs (MRT = raw ES 3.00) |
| 8-lane `uChannel` scatter maps | **`shadow-hot` RGB**, 3 screen-space lanes | D-2 → more lanes |
| casters = **textured earcut silhouette** (`outline`) + UV alpha | **solid billboard quad** | D-3 → the 5-tri fan + UV alpha |
| cold lights from a **per-rect light-data texture** | **6→24 debug lights** (uniforms) | D-4 → per-rect texture, real content lights |
| `shadow-cold` a **bake input** to `lightmap-cold` (never displayed) | inspected **directly** via the overlay (no lit display yet) | consumed by the lit display once lighting returns |
| "cold" = static baked | **"cold" = round-robin** (warm behaviour), provisional name | D-6 → rename/split cold vs warm |

The screen-hot/world-cold split (row 1) is the load-bearing *new* idea — it's what makes the counts
scalable without the rect-fighting that sank the last attempt. The rest are count-shrinks.

## Current ground truth (post-nuke) this builds on

- **Kept:** the G-buffer `SquareCache` (cold+warm tiers; albedo/normal/surface/zdepth per-prim bakes,
  dirty-tracked, **toroidal** world-space window + reproject-on-zoom + the 4-way wrap apron), the unlit
  `albedoBlitShader` display, `/showRT` + `/overlayRT` via `overlayShader` + `overlayModeFor`.
- **Gone (this stream re-adds, minimally):** any light source (`LightRig` deleted — need a small light
  list), any shadow machinery, `standingPrims()` (the caster enumerator). Rebuilt clean, not restored.
- **Reused, not rebuilt:** the toroidal window→buffer wrap math is exactly the 4-copy this stream needs —
  lift it from the cache's existing apron/reproject rather than reinventing it.
- **Platform:** the shadow shaders are **GLSL ES 1.00** (Pixi's high-shader compiles ES 1.00 even on the
  WebGL2 context — [`design/rendering-platform.md`](../../components/client/pixijs/design/rendering-platform.md),
  [F14](forks.md#f14), proven by [`bitfield-rt`](../bitfield-rt/issues.md#i-8)): **float-mod** bit
  pack/decode on a **unorm RGBA8** `shadow-cold`. ES 3.00 (integer textures, MRT) is deferred behind
  hand-written raw shaders.

## State

Phased in [`todo.md`](todo.md); decisions in [`forks.md`](forks.md); intentional design deltas in
[`deviations.md`](deviations.md); anticipated technical gotchas in [`issues.md`](issues.md). The durable
target outlives this folder in the component `intent/`/`design/` — this stream graduates its proven
mechanism into those as it stabilises.
