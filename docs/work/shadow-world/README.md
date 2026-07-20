# Work — shadow-world (world-space shadow-a/b + `/overlayRT` debug display)

_Opened 2026-07-20. Builds directly on the archived **shadow-cast** experiment (cast 5 lights' shadows
into a ping-pong bitfield + incremental updates — proven). This one moves the bitfield to **world space**
and swaps the built-in colour display for a **`/overlayRT`** debug view. Component:
[`client/pixijs`](../../components/client/pixijs/). This is essentially building the real
[`shadows`](../shadows/README.md) `shadow-cold` storage — a world-space, toroidal, ping-ponged bitfield._

## Why — the two changes from shadow-cast

`shadow-cast` proved the mechanism (casting into a bitfield + incremental per-light updates) but stored
the bitfield in **screen space** and drew the colours with a bespoke full-viewport display. Two changes
make it the real thing:

1. **World-space `shadow-a` / `shadow-b`** — put the bitfield RTs in the **same toroidal world-space
   layout the other composites use** (`albedo-cold`, `normal-cold`, …): same fixed buffer, same window /
   slot geometry, same pan + nearest-reproject-on-zoom. Shadows live on the **ground** (world), so storing
   them world-space means they stick to the world as the camera pans/zooms — which is what `shadow-cold`
   must do.
2. **Display via `/overlayRT`, not a built-in mesh** — **remove** the bespoke colour display; instead
   decode the bit-colours in the **overlay** so `/overlayRT shadow-a` (or `shadow-b`) paints the coloured
   shadows, world-aligned, as a debug tool alongside every other G-buffer channel. The colour decode is a
   great inspector; it belongs in the same `/overlayRT` path as `albedo`/`normal`/`zdepth`.

Everything else carries over from shadow-cast unchanged: **5 lights, 5 bits (RED byte), billboard
shadows from in-radius prims, one light moves/second, re-cast only the dirty light** (cast → mask,
combine mask + src → dest, ping-pong), and the **light dot + radius-ring markers** (kept — they're the
verification aid).

## What changes concretely

- **Cast in WORLD space.** Project each in-radius prim's billboard from the light to the **ground plane
  (z = 0) in world coords** (drop the world→screen step) → a world-space shadow quad. Rasterise into a
  **world-space `mask`** in the cache's fixed buffer (toroidal), via the same window→slot mapping the
  composites bake through.
- **`shadow-a` / `shadow-b` = world-space RTs** sized to the cache's fixed buffer (`fixedCW × fixedCH`),
  sharing its window origin + `slotPx` + pan, ping-ponged. The **combine** reads world-space `src` + world
  `mask` → writes world `dest` (clear bit k, set where masked, carry the other 4 bits); swap.
- **Toroidal write** — a world shadow quad can straddle the buffer seam, so the write wraps (reuse the
  cache's window→buffer wrap; see [shadows F10](../shadows/forks.md#f10)). *Simplification allowed for
  bring-up:* write only the on-screen window first and add the wrap once the basics read right
  ([F3](forks.md#f3)).
- **Zoom** — `shadow-a`/`-b` reproject with the cache window **nearest** (a bitfield can't bilinear-
  resample), then the next re-cast refills — same rule as `shadow-cold` ([shadows I-1](../shadows/issues.md#i-1)).
- **Expose to `/overlayRT`** — list `shadow-a`/`shadow-b` (the current buffer) in
  `Viewport.renderTextures()`; add a **bit-decode overlay mode** to `overlayShader` (float-mod, 5 colours,
  additive) routed for `shadow-*` names in `overlayModeFor`. **Delete** the standalone `ShadowDisplayShader`
  + its display mesh.

## What it proves

- **World-space bitfield storage** — the shadows stick to the world (pan/zoom) instead of the screen, the
  way `shadow-cold` must.
- **Casting + incremental updates survive the move to world-space + toroidal** — the shadow-cast result
  holds when the bitfield is a world composite, not a screen RT.
- **`/overlayRT` is the debug surface** — the coloured-shadow inspector lives in the same overlay path as
  the rest of the G-buffer, world-aligned.

## Latitude

Casters = the cold cache's `standingPrims` (things) in radius (as shadow-cast). Shadow shape = the
billboard quad (no silhouette yet — [shadows D-3](../shadows/deviations.md#d-3)). Bits = RED byte, 5
lights; **A never used for data** ([`rendering-platform.md`](../../components/client/pixijs/design/rendering-platform.md)).
Reuse the shadow-cast code (`shadowCast.ts` / `shadowCastShaders.ts`) — this is an edit of it, not a
rewrite.

## State

Phased in [`todo.md`](todo.md); decisions in [`forks.md`](forks.md); gotchas in [`issues.md`](issues.md).
On PASS the world-space storage + `/overlayRT` decode graduate into [`shadows`](../shadows/README.md), and
this folder archives (to `../archive/`, as shadow-cast did).
