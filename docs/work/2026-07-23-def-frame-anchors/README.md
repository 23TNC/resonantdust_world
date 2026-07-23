# Def frame/anchor rework — 2026-07-23

_Component: [`client/webgl`](../../components/client/) · `game/viewport/coldShadowData.ts` +
`shadowGather.ts` (+ later `thingPlacement`/DSL size sourcing). Phases in [`todo.md`](todo.md);
layout amendments are [`forks.md`](forks.md)._

Rework `prim_definition_data` onto the **whole-px-per-unit** model: frames are **pow2 squares**, so
the def stores a **lod exponent** instead of a width/height pair, the **minimum bbox lives in world
units**, and a **px nudge** aligns the sampled window to the opaque pixels. Adds **3×3 frame
anchors** so a prim's reported x/y can carry the bbox at any of top/middle/bottom × left/center/
right. Supersedes the P4-era opaque-sub-rect fields (frame_x/y/w/h in 16-px grid units).

## Invariants (the model)

1. **Whole px per unit.** Minimum texture size is **1 px/unit** → smallest frame is 16×16 (lod 4).
   ppu **doubles** per lod: `ppu = 2^frame_lod / span_units` where `span_units = 16 · 2^frame_span`
   (frame_span = log2 tiles of the frame's world span). Always a whole pow2 ≥ 1 — no 2.25-px units,
   ever. (NOT `frame_lod − 3`: lod 4→1, 5→2, 6→4, 7→8 …)
2. **Frames are pow2 squares on a 16-px atlas grid.** Per-LOD-size pools pack uniform pow2 ≥16
   squares with zero padding (MaxRects degenerates to grid placement) → every frame origin is
   16-aligned. A future mixed-size quadtree packer preserves this (pow2 cells ≥16 are grid-aligned).
3. **Ceilings.** Per-prim texture ≤ **1024²** (one zone at SQUARE=64); GL max page 16384 → u10
   16-px-grid coords; span ≤ 16 tiles → u3 log2; theoretical max ppu (lod 14, 1-tile span) = 1024.
4. **Bbox in EVEN units** (2-unit increments) so half-anchor shifts are integral units.
5. **Bottom alignment.** The nudge y-aligns opaque px to the bbox BOTTOM (shadows anchor at the
   base); x centers them.

## Layout (amended — [F1](forks.md#f1), [F2](forks.md#f2))

```
R  u32  u9 prim_width (23–31, 2-unit steps) | u9 prim_height (14–22, 2-unit steps)
        | u4 frame_span (10–13, tiles − 1; width = log2(ZONE_DIM) — F1 as RATIFIED) | u10 reserved (0–9)
G  u32  u10 offset_x (22–31, units) | u10 offset_y (12–21, units) | u12 reserved (0–11)
        (UNSIGNED, frame-relative: bbox top-left indexed into the frame from frame_x/y — no ±512 bias)
B  u32  u10 frame_x (22–31, 16-px grid) | u10 frame_y (12–21, 16-px grid) | u4 frame_page (8–11)
        | u4 frame_lod (4–7, side = 2^lod) | u2 frame_anchor_x (2–3) | u2 frame_anchor_y (0–1)   FULL
A  u32  u12 nudge_x (20–31, px, signed +2048) | u12 nudge_y (8–19, px, signed +2048)
        | u2 nudge_anchor_x (6–7, default 1 center) | u2 nudge_anchor_y (4–5, default 2 bottom)
        | u4 reserved (0–3)   FULL (F4 as ratified)
```

- `prim_width/height` — the minimum bbox in units, **even** (stored /2). u9 = ≤1022 units (~4-zone
  headroom over the 256-unit ceiling).
- `offset_x/y` — bbox top-left within the frame, units. Sample px = `frame_xy·16 + offset·ppu + nudge`.
- `frame_lod` — pow2 side exponent (4..14). Replaces frame_width/height (saves 20 bits): exact
  because frames are square pow2.
- `frame_anchor_x/y` — 0 none | 1 half | 2 full shift of the bbox against the prim's reported x/y
  (draws left→right, top→bottom from the anchored origin; both sides in units so placement is direct).
  Shadows use bottom-center = (1, 2). Flip (rotation=W) mirrors anchor_x (0↔2).
- `nudge_x/y` — px, biased top-left bbox → nudge right until opaque px are x-centered, up until
  bottom-aligned. Bound: 2 units × max ppu 1024 = 2048 → u11. Stored at the CURRENT lod's px scale —
  recomputed by the def compare-write when the lod swaps.

## Sampling chain (GPU)

```
ppu      = 2^frame_lod / (16 · 2^frame_span)
bbox px  = prim_wh (units) · ppu
uv(s,t)  = frame_xy·16 + offset_xy·ppu + vec2(nudge_x, −nudge_y) + (s, 1−t)·bbox_px
```
(s,t) from the proven point-in-quad inversion, in-range by construction — unchanged from P4.

## Why

The 16-px-grid opaque-rect scheme kept sampling exact but let ppu go fractional against the raw
frame (36 px / 16 units = 2.25) and burned 20 bits on a redundant w/h (pow2 squares need one
exponent). This model makes whole-px-per-unit a **type-level invariant**, frees ~2 more bits net
after adding span, and buys 3×3 anchor placement for free.
