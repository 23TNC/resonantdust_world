# Strip the lighting + shadow system — 2026-07-31

_Component: [`client/webgl`](../../components/client/webgl/). Plan in [`todo.md`](todo.md); decisions in
[`forks.md`](forks.md); what we keep from the wreckage in [`issues.md`](issues.md)._

## The user's instruction

> "We are going to completely strip our lighting and shadow system, as we need to re-think how we are
> handling it."

So this stream **removes**, and removes cleanly. It does not design the replacement — that is the
user's to author, and pretending otherwise here would bake today's assumptions into tomorrow's
system, which is the specific failure the strip exists to escape.

## What is being removed

| | lines / size | |
|---|---|---|
| `shadowGather.ts` | **3 000** | the gather, the lighting bake, the receiver bake, the decay/splat pass, ~10 GLSL programs, ~25 debug hooks |
| `coldShadowData.ts` | 1 348 | the unified data-texture writer — **partly** lighting (see [F5](forks.md#f5)) |
| lighting RTs | **~83 MiB** | 4 shadow (2 MiB ea) + decay (1) + receiver coarse (2) + receiver fine (8) + 2 lightmaps (**32 MiB ea**) |
| `albedoBlitShader.ts` | — | the `albedo × (ambient + cold + hot)` composite reduces to `albedo × ambient` |
| `Viewport.ts` | — | the per-frame draw sequence loses **6 of its 8 steady-state draws** |
| docs | 4 files | `design/{lighting,shadows}.md`, `intent/{shadows,tiered-lighting}.md` |
| `VARIABLES.md` | 3 bands + 1 RT spec | `light_data`, `light_presence_lo`, `light_presence_hi`, `shadow-cold` |

## Why strip rather than keep patching

The system works — it is not broken, and that is worth saying plainly. What it is, is **structurally
committed**: eight streams of decisions have narrowed the design space to the point where every
remaining improvement runs into something that was settled two rewrites ago. The evidence is in
[`issues.md`](issues.md), and it is specific:

- **A shadow texel is 128 bits and every extension needs more.** A caster reference is `u20`; 8 of
  them is 160. Every route to a second px costs something real — MRT hung Chrome twice and was never
  root-caused, a 2×-wide RT runs the walk (87 % of the pass) twice, and a derive pass is redundant
  while every caster is a billboard.
- **`max` keeps HOW MUCH and discards WHICH**, which cost a 9.29 ms refine of a 10.88 ms pass before
  it was deleted. The id map fixed that — and immediately hit the 128-bit ceiling above.
- **Penumbra was computed and thrown away** for an unknown number of streams. A histogram found the
  map perfectly bimodal: 31 155 shadowed samples, **zero** partial.
- **Z-order is resolved per pixel in the blit**, so layering is a fragment cost rather than a draw
  order, and adding a layer means adding a comparison to the hottest shader in the frame.
- **The differential was never wired.** `DIFFERENTIAL_WIRED = false`; `coldShadowPrevRT` and
  `hotShadowPrevRT` have been allocated, never written and never read, idling 4 MiB.

None of those is a bug. They are the shape the system settled into, and a re-think is the honest
response.

## What survives, and why

The strip is **surgical, not scorched-earth** — several things live in the lighting files without
being lighting ([F5](forks.md#f5)):

- **The primitive graph** (`prim_data`, `billboard_data`, `definition_data`) — this is how the world
  addresses objects. Sprites, z-depth and selection all read it.
- **The G-buffer** (`SquareCache`, the fixed 32×16 slot grid, the mrt-bake) — that is *rendering*,
  and the display blit needs it whatever lights it.
- **The art pipeline's normal and depth maps.** They exist to be lit, so nothing consumes them after
  the strip — but they cost nothing to keep and any successor will want them ([F4](forks.md#f4)).
- **F8 self-positioning records**, landed hours ago: a billboard root's RED is now its whole
  `region|zone|tile|unit` address, so any consumer holding a bare id can place it. That is a general
  record improvement that the lighting work merely *provoked*.

## The one thing this stream will not decide

**What replaces it.** [`blockers.md`](blockers.md) states the question rather than answering it. The
strip is designed so the answer is not needed to proceed: it ends at a renderer that draws unlit
albedo, with the seam documented and measured.
