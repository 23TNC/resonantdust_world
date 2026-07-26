# TEXTILE_SLOT — fix every map to a slot grid, reproject on zoom — 2026-07-26

_Component: [`client/webgl`](../../components/client/) · `game/viewport/` — `SquareCache` channels,
`shadowGather` light/shadow RTs, `squareMath` resolutions, `definition_data`/`light_data`/`billboard_data`.
Phases in [`todo.md`](todo.md); decisions in [`forks.md`](forks.md); findings in [`issues.md`](issues.md)._

## The decision (user, 2026-07-26)
**Every textile map is sized in TILES, not in screen resolution, and never changes size.** A fixed grid of
**24×16 SLOTS** (20×12 visible + 2 slots of overscan per side). A slot holds **1 tile at lod 0** and
**2^k × 2^k tiles at lod k**, so the texture is constant while the world it covers grows by 4× per lod step.

**`SQUARE` becomes 128** (was 64). Max art size is 128px — there is never a reason to author 256px when the
slot can't show it.

## Why — it fixes three unrelated problems at once
1. **Memory stops tracking zoom.** Today the lightmap is world-sized at 64 texels/tile, so zooming out grows
   it: measured **87.8 MB per tier, 176 MB for both**, larger than the entire 8-surface G-buffer (~110 MB)
   while carrying less information ([primitive-graph I31](../2026-07-25-primitive-graph/issues.md#i31)).
2. **Gameplay stops tracking monitor.** An 8K player currently sees more world than a 1080p player. Fixing
   tiles-at-zoom-1 makes the view identical everywhere, and caps the art pipeline as a side effect.
3. **Zoom stops flashing.** `SquareCache.ensurePartition` clears every channel on re-partition — its own
   comment admits it ("no reproject — simplified W4c"). A tile-aligned pow2 grid makes reprojection trivial
   arithmetic instead of a resample problem.

## The grid
| | |
|---|---|
| slots | **24 × 16** (20×12 visible, 2 overscan per side) |
| texels per slot (square family) | **128** |
| slot texture (square family) | **3072 × 2048** |
| reference render target | **2560 × 1536** (the 20×12 visible slots at lod 0) |
| lod levels | **4** → `frame_lod` fits in **u2** |

**The same slot grid serves every map; only texels-per-slot differs.** That is what lets `lod`,
`slot_width`, `slot_height` live once in the data-texture constants and be read by every shader — which
makes the recurring "recomputed a `textile_slot` address from a world coord" bug
([map-compatibility](../2026-07-24-map-compatibility/README.md)) *unexpressible* rather than merely
discouraged.

| family | texels/slot | dims | example maps |
|---|---|---|---|
| square (per px at lod 0) | 128 | 3072×2048 | albedo, normal, surface, zdepth, lightmap |
| unit (per unit) | 16 | 384×256 | shadow |
| tile (per tile) | 1 | 24×16 | presence, caster buckets, dirty |

**Alignment worth leaning on:** at lod 3 a tile occupies 128/8 = **16×16 texels**, exactly the unit
resolution (16 units/tile). At maximum zoom-out the square-family map and the unit-family map are in 1:1
correspondence.

## Scale, zoom and lod
**Fit is COVER, not contain** — the viewport must land entirely inside the visible slots:
```
s = max(W / 2560, H / 1536)
```
`max` (not `min`) is what guarantees no overscan or empty region is ever on screen. Taking `σ ≥ s` as the
live scale and `zoom ≡ σ / 2^k`, each lod covers a 2× band:

| lod | tiles/slot | tile texels | visible tiles | zoom band (at s=1) |
|---|---|---|---|---|
| 0 | 1×1 | 128 | 20×12 | [1, 2) |
| 1 | 2×2 | 64 | 40×24 | [0.5, 1) |
| 2 | 4×4 | 32 | 80×48 | [0.25, 0.5) |
| 3 | 8×8 | 16 | 160×96 | [0.125, 0.25) |

**Two sampling stages, and they behave differently** ([I1](issues.md#i1)):
- **Bake (art → slot)** is always exactly 1:1 — the art mip at lod k is `128/2^k` px and the slot footprint
  is `128/2^k` texels. Never up- or downsampled.
- **Display (slot → screen)** has ratio **`1/σ`**, which is *independent of lod* — the `2^k` cancels. So lod
  selects world coverage only; it does not affect sharpness.

Consequently a downsample-only display would need 4× the texels (40×24 visible slots, ~77 MiB per RGBA8
surface). **Rejected** — [F2](forks.md#f2) takes continuous zoom with up to 2× magnification within a band,
snapping back to 1:1 at each lod boundary.

## Memory (fixed, at every zoom, on every monitor)
| surface | dims | format | size |
|---|---|---|---|
| albedo, normal, surface, zdepth × cold+warm (8) | 3072×2048 | RGBA8 | 192 MiB |
| lightmap (single accumulator, hot/cold collapsed) | 3072×2048 | RGBA32F | 96 MiB |
| shadow (2 attachments) | 384×256 | RGBA32UI | 3 MiB |
| **total** | | | **~291 MiB** |

Against ~374 MiB today at zoom 0.25 — a reduction that also stops moving. The per-channel figure rises
(14 → 24 MiB) only because the reference target is now 2560×1536 rather than this machine's 1862×853 panel:
**that is a bought quality/uniformity standard, not waste.** Do not "optimize" it back down.

## Reproject instead of clear
On a lod change, **rescale; do not re-bake**. Zoom in → retained tiles are already present, upscale and clip.
Zoom out → downscale the retained tiles into the same texture and mark only the *newly covered* tiles dirty.

**NEAREST is the universal rescale rule** ([F3](forks.md#f3)) — replication up, decimation down, never
averaging. It is independently *mandatory* for three separate reasons: the shadow bitfield cannot be filtered
at all ([shadows I-1](../shadows/issues.md)), `zdepth` encodes a discrete `0x80 | baseRow` that averaging
silently corrupts, and the additive lightmap needs exact integers to stay invertible. Linear remains
available as an optional quality choice for albedo/normal/surface only.

## Per-piece lod state — two fields, different jobs
- **`billboard_data.last_lod` (u2)** — the lod last rendered at, for def-swap detection. Billboards are
  re-baked, not accumulated, so "last" is correct.
- **`light_data.coarsest_lod` (u2)** — the **coarsest** lod since the light was last cast, for exact
  subtraction. "Last" is *wrong* here: a down-then-up round trip is lossy, so the stored contribution lives
  on the coarsest grid it ever passed through ([I2](issues.md#i2)).

Per-light (not per-tile) lod state is what makes zoom cheap: a tile may hold contributions deposited at four
different lods simultaneously, because each light independently knows the grid it must be undone on. No
forced conversion of 16 lights per tile on a zoom.

## Refinement, prioritised
A lod change makes content stale, not wrong. Refinement is a queue, and **empty beats stale** so a player
never sees a hole:
1. **empty** — `slotBaked === 0`, never populated. Highest priority.
2. **stale lod** — a billboard whose def lod ≠ current, or a light whose `coarsest_lod` ≠ current.

Without the light half of that queue, a light that has been zoomed out and back in stays permanently blocky.
The billboard half is largely wired already: `definitionFor` → `billboardDataFor().changed` →
`markBillboardDirty`. The work is removing the blanket invalidation, not building the refinement.

## Relationship to other streams
- **Gates [primitive-graph F11b](../2026-07-25-primitive-graph/forks.md#f11b)** (additive RGBA32F lightmap):
  at world-fixed density RGBA32F costs 465 MB; on the slot grid it costs 96 MiB.
- **Subsumes** [webgl-engine](../webgl-engine/README.md) W4h's "restore the `Channel` ping-pong / reproject"
  one-liner, and generalises it from the G-buffer channels to every map.
- **Resolves** [map-compatibility](../2026-07-24-map-compatibility/README.md) structurally — publishing the
  slot mapping as constants removes the class of bug that stream exists to police.
- **Re-anchors** [lightmap-resolution](../2026-07-24-lightmap-resolution/README.md): that stream deliberately
  chose `TEXTILE_SQUARE` to spend memory on sharpness. This keeps the sharpness where it is visible (1:1 at
  each lod boundary) and stops paying for it where it is not.
