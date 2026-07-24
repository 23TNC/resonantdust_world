# Full-resolution lightmap — cache per-light lighting at normal res, for many lights — 2026-07-24

_Component: [`client/webgl`](../../components/client/) · `game/viewport/` — the baked lightmap RTs +
`LIGHT_FRAG` + the display blit. Phases in [`todo.md`](todo.md); decisions in [`forks.md`](forks.md);
findings in [`issues.md`](issues.md). Sibling of [`2026-07-24-world-space-lighting`](../2026-07-24-world-space-lighting/README.md)
(that makes the light *correct* — world-space 3D `N·L`); this makes it *sharp + scalable*._

## The decision (user, 2026-07-24)
The game will have **a lot of lights**, and its detail lives in the **lighting**, not the albedo. So:
**bake a `normal`-resolution lightmap (cold + hot) that caches the accumulated per-light lighting.** Spend
the GPU memory — freed up by the recent max-zoom 4×→2× reduction (the fixed cache reserve `RESERVE_CSS`
scales with `ZOOM_MAX`) — to buy back the compute, because a lighting-driven game is where that trade pays.

## Why CACHE, not forward-every-frame (the many-lights calculus)
The discussion established that forward per-light lighting (loop lights per pixel, `Σ color·falloff·
(1−shadow)·max(0,N·L)`) is *correct* and cheap for ~14 lights. But it runs **every frame, every pixel**. With
**many** lights that scales with light-count × pixels × framerate. The **bake** amortizes it: the per-light
loop runs once per **dirty** tile (static lights → never re-bake), and the blit is a single read. So caching
is what makes "a lot of lights" affordable — the expensive per-light accumulation happens rarely, not per
frame.

## Cache the CORRECT thing — bake `N·L` per light, at normal res
The trap we must avoid ([issues.md#i1](issues.md#i1)): today's coarse lightmap pre-sums the lights into one
`irradiance` + one **averaged** direction, then applies the normal once in the blit — and `Σ colorᵢ·(N·Lᵢ)`
does **not** equal `(Σ colorᵢ)·(N·L_avg)`. That averaging is lossy at any resolution. The fix is to do the
`N·L` **per light, inside the bake**, against the **fine normal** — so the lightmap stores the fully
accumulated, per-light-correct irradiance:
```
lightmap[fine texel] = Σ_lights color · falloff · (1 − shadow) · max(0, N · dir_to_light)
```
The blit then collapses to `out = albedo × lightmap` (+ ambient). No averaging, no per-frame light loop.
Bonus: with `N·L` baked in, the separate **direction** (att1) and **unshadowed** (att2) attachments go away —
the fine lightmap is a single irradiance RGB per class, so it's ~1 attachment where we had 3
([forks.md#f5](forks.md#f5)).

## The central technical challenge — the fine normal inside the bake
To bake `N·L` at fine res, `LIGHT_FRAG` needs the **normal at each bake texel**. The lightmap bake is a
**contiguous world-space toroidal** map (`textile_unit` family); the normal today lives in the `SquareCache`
**composite**, a `textile_slot` atlas that is NOT safe to address by recomputed world coordinate (the
[map-compatibility](../2026-07-24-map-compatibility/README.md) lesson — the exact zoom-drift that reverted
shadows-on-prims twice). So we need the fine normal in a **contiguous, world-coord-addressable** form for the
bake ([forks.md#f3](forks.md#f3), the key open decision): most likely bake the normal into a contiguous
world-space map aligned with the lightmap (a second normal target, or restructure the normal bake), NOT
sample the slot atlas from the bake. This is the crux and where the risk lives.

## Resolution + family
Target `R` = **`TEXTILE_SQUARE` = 64/tile** (matches the composite's max detail), **contiguous** `cols·R ×
rows·R` toroidal — NOT the composite's variable-`slotPx` `textile_slot` atlas ([forks.md#f4](forks.md#f4),
[issues.md#i3](issues.md#i3)). 4× per axis / 16× the texels of today's 16/tile — but partly offset by
dropping 3 attachments → 1, and by HDR only where needed ([forks.md#f5](forks.md#f5)). The **shadow map
stays coarse** (16/tile) — the gather is the expensive pass; the fine bake **upsamples** `shadow-cold` per
light. The **albedo** stays on the composite (untouched; coarsening it is a later option).

## What it composes with
- [world-space-lighting](../2026-07-24-world-space-lighting/README.md): `dir_to_light` becomes the true 3D
  vector per light, dotted against the **pitched** fine normal (`normal-tilt`) — the per-light `N·L` this
  bake performs is exactly where that correctness lands. These three streams converge here.
- The shadow gather + `shadow-cold` are **unchanged** (still 16/tile, corridor↔brute identity intact) — we
  only change how lighting is *accumulated + stored*, not how shadows are *computed*.
</content>
