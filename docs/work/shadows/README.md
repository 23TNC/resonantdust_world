# Work — shadows (the cold-shadow bitfield foundation)

_Opened 2026-07-19. The clean restart after the tiered-lighting port was nuked (the reverted
`lighting` stream, archived out-of-repo). This stream builds the **shadow RT +
bit-packing pipeline** from scratch, at the smallest scale that exercises every moving part, so the
full design can grow on top of a working, understood core. Component:
[`client/pixijs`](../../components/client/pixijs/). Builds toward
[`intent/tiered-lighting.md`](../../components/client/pixijs/intent/tiered-lighting.md) (the shared
scatter→bitfield shadow engine) and [`design/shadows.md`](../../components/client/pixijs/design/shadows.md)
(the billboard-quad projection)._

## Why — one honest slice of the shadow engine, end to end

The nuked attempt tried to land the whole tiered engine (32-bit bitfield, 8-lane scatter maps, textured
silhouettes, cold/warm/rt, the lit display) at once, and never got a clean result. This stream inverts
that: build the **thinnest vertical slice** that still touches every hard part —

- a **`shadow-hot` RT** that stages a few lights' shadows in separate channels,
- a **`shadow-cold` RT** that is a **packed bitfield** (one bit per light),
- a **project → stage → pack** loop that fills the bitfield in batches, and
- a **decode overlay** that proves what's in the bitfield, per-bit.

Everything else (32 bits, per-rect light textures, textured silhouettes, the lit display) is a
*widening* of this slice, deferred on purpose. **This is the foundation the more complex operations
build off of.**

## The slice (target of THIS stream)

**Two RTs.**

- **`shadow-hot`** — an RGBA8 staging RT. **RGB = three shadow lanes** (one light each); **A unused**.
  Written fresh per batch, consumed immediately by the pack, then reused for the next batch. (Data in
  RGB, never A — A is the premultiply/opacity lane and unreliable for data; see [F2](forks.md#f2).)
- **`shadow-cold`** — an RGBA8 bitfield RT. **RED channel = an 8-bit occlusion bitfield** (bit `i` =
  light `i` casts a shadow here); **A held at 1** so the premultiply path never corrupts the RED byte
  (see [D-1](deviations.md#d-1)). This stream uses **6 of the 8 bits**.

**The loop.** Seed **6 cold lights** around tile **(100, 50)**. Then, in **two batches of three**:

1. Project batch's 3 casters' shadows → write each into its **own `shadow-hot` channel** (light→R, light→G, light→B).
2. **Pack** `shadow-hot`'s RGB into three **bits** of `shadow-cold`'s RED byte (batch 0 → bits 0/1/2,
   batch 1 → bits 3/4/5), preserving the bits already set.

Two batches ⇒ `shadow-cold` holds **6 shadows, one bit each**.

**Casters are pure billboards.** A caster is the standing prim's **billboard quad** (W×H box, tilted by
the ground angle), projected radially from the light to the ground per
[`design/shadows.md` §Projection](../../components/client/pixijs/design/shadows.md) — drawn as a **solid
2-triangle quad**. **No texture sampling, no alpha mask, no `outline` silhouette** — those are the next
layer, deliberately out of scope (see [D-3](deviations.md#d-3)).

**The overlay proves it.** `/overlayRT shadow-cold` gets a new decode: read the RED byte, split its 6
bits, and paint **each bit a unique colour** (6 colours for 6 bits). **Overlapping shadows combine
colours** (additive), so where lights 0 and 1 both shadow a spot you see colour 0 + colour 1.

## How this maps onto the target design (and where it intentionally shrinks)

The [tiered-lighting](../../components/client/pixijs/intent/tiered-lighting.md) engine is this slice,
widened. The gaps are deliberate and pre-logged in [`deviations.md`](deviations.md):

| target design | this foundation | widened later |
|---|---|---|
| `shadow-cold` = **32-bit** bitfield (full RGBA texel) | **RED byte**, 6 bits used | D-1 → pack across all 4 bytes for 32 |
| scatter via **2 maps × 4 `uChannel` lanes** (8) | **`shadow-hot` RGB**, 3 lanes | D-2 → the 4-lane/`uChannel` maps |
| casters = **textured earcut silhouette** (`outline`) w/ UV alpha | **solid billboard quad** | D-3 → the 5-tri fan + UV alpha of `design/shadows.md` |
| cold lights from a **per-rect light-data texture** | **6 debug lights** (uniforms) | D-4 → the per-rect texture w/ real content lights |
| `shadow-cold` is a **bake-time input** to `lightmap-cold` (display never reads it) | `shadow-cold` inspected **directly** via the overlay (no lit display yet) | the lit display consumes it once lighting returns |

None of these shrink the *mechanism* being proven — projecting, staging in channels, packing to bits,
decoding — they only shrink the counts. That's the point: get the mechanism right small, then scale it.

## Current ground truth (post-nuke) this builds on

- **Kept:** the G-buffer `SquareCache` (cold+warm tiers; albedo/normal/surface/zdepth per-prim bakes,
  dirty-tracked, toroidal), the unlit `albedoBlitShader` display, `/showRT` + `/overlayRT` via
  `overlayShader` + `overlayModeFor`.
- **Gone (this stream re-adds, minimally):** any light source (`LightRig` deleted — need a small cold-light
  list), any shadow/scatter machinery, the derived/post-pass channel hook, `standingPrims()` (the caster
  enumerator). All rebuilt clean and simple here, not restored.

## State

Phased in [`todo.md`](todo.md); decisions in [`forks.md`](forks.md); intentional design deltas in
[`deviations.md`](deviations.md); anticipated technical gotchas in [`issues.md`](issues.md). The durable
target outlives this folder in the component
`intent/`/`design/` — this stream graduates its proven mechanism into those as it stabilises.
