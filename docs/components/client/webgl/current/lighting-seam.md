# Where a lighting system attaches

_Last updated: 2026-07-31 · verified @ b365a79f. Written by
[`work/2026-07-31-lighting-strip`](../../../../work/2026-07-31-lighting-strip/README.md) P5, so its
successor starts from measurements rather than an empty folder._

## The frame, and where lighting goes in it

The steady-state frame is **one draw**: the display blit over the G-buffer composites.

```
  SquareCache bake  ──►  G-buffer composites (cold + warm)     dirty-gated; idle once settled
                              │
                         ◄────┴──── A LIGHTING SYSTEM ATTACHES HERE ────►
                              │      reads the composites, writes its own target(s)
                              ▼
  display blit      ──►  screen                                albedo.rgb × alpha
```

The blit is the **only** per-frame draw in the steady state, and it is where a lightmap multiply would
land. `Viewport.tick()` is the whole frame; the removed system was driven from a single
`shadows.tick(...)` call in it, immediately before the blit.

## The budget

| | ms/frame | draws |
|---|---|---|
| **today, unlit** | **0.028** | 1 |
| the removed system, static scene | 0.508 | 4 |
| the removed system, one moving light | 0.675 | 4 |

Measured at `?user=Claude&focus=100,50&zoom=1`, 60 frames after 15 warm-up, `gl.finish()` at both
ends, median of 5. The old lighting was **~91 %** of a static frame, so a replacement has roughly
**0.48–0.65 ms** before it costs more than what came out — at this scene's density, with one authored
torch. Not the N=16 stress case.

**Do not use `EXT_disjoint_timer_query_webgl2` to measure this.** The debug tab runs
`document.hidden`, so rAF never fires and `setTimeout` is throttled to ~1 s; timer queries then
retire unreliably and **return plausible partial data rather than failing**
([I7](../../../../work/2026-07-31-lighting-strip/issues.md#i7)). Wall clock with `gl.finish()` and
hand-driven `__viewport.tick()` is the working recipe.

## What is available to read

Per frame, on the fixed **32×16 slot grid**, in two tiers — cold (static) and warm (movers),
composited warm-over-cold:

| channel | lane | carries | live? |
|---|---|---|---|
| `albedo` | rgb | base colour | **read by the blit** |
| `surface` | R / G / B | presence / ambient occlusion / alpha coverage | B read; **G unread** |
| `zdepth` | B | the painter's key `0x80 \| (baseRow & 0x7f)` — resolves warm-over-cold per pixel | **read by the blit** |
| `zdepth` | R | relayed the emissive mask | **unread** |
| `normal` | rgb | per-texel surface normal | **unread** |

**Normal and depth maps are still generated for every master** by the art pipeline and nothing
consumes them. Kept on purpose ([F4](../../../../work/2026-07-31-lighting-strip/forks.md#f4)): any
lighting model worth building wants a surface normal, and regenerating the corpus is hours of GPU time
plus re-tuning. The risk is that unconsumed maps **drift** — nothing renders them, so nothing catches
a regression.

## What is NOT there any more

There is no data texture. The unified `RGBA32UI` texture and every band on it — `definition_data`,
`prim_data`, `billboard_data`, `light_data`, both `light_presence` sets, `prim_presence` — went with
`coldShadowData.ts`. A replacement builds its own record layer; it does not inherit one.

Also gone: ~99 MiB of render targets, the hot/cold tier split and its class-aware dirty machinery, and
`DIFFERENTIAL_WIRED` (which was never `true`).

## The one thing the strip added

**`Viewport.tightBoxFor`** reads `TextureResolver.opaqueBBox(stem)` directly for click hit-testing.
It used to route through the record layer, and that was the record layer's *only* non-lighting reader
([D2](../../../../work/2026-07-31-lighting-strip/deviations.md)). A new lighting system does **not**
need to serve selection.

## The acceptance a replacement inherits

What the removed system actually did, so "does the new one do X" is answerable rather than argued:

1. Point lights with soft radial falloff, authored per content kind
2. Projected silhouette shadows, direction per caster from angle-to-light
3. Shadows onto billboards as well as ground (the climbing shadow)
4. n/s perpendicular caster cards for rotated billboards
5. Movers lit and casting in the same pass as static geometry
6. Per-light N·L against a normal map
7. Emissive (self-lit) pixels
8. Ambient × AO on the omnidirectional term
9. Decay / flicker glow
10. Bilinear shadow upsample

**7–9 are explicitly not in the successor design** — a decided loss, not an oversight
([rework F11](../../../../work/2026-07-31-lighting-rework/forks.md#f11)).

The four reference renders of the old system are in
[`work/2026-07-31-lighting-strip/before/`](../../../../work/2026-07-31-lighting-strip/before/), with
the unlit equivalents beside them in `after/`.

## Where the successor is designed

[`intent/2026-07-31-rework.md`](../../../../intent/2026-07-31-rework.md), built by
[`work/2026-07-31-lighting-rework`](../../../../work/2026-07-31-lighting-rework/README.md).
