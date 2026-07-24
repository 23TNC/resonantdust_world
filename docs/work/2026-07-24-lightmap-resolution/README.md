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

## The fine normal inside the bake — RESOLVED (sample the prim's atlas frame)
To bake `N·L` at fine res, `LIGHT_FRAG` needs the normal at each bake texel — and there is **no world normal
map** needed. Because the bake is doing **per-prim** lighting, it already finds the prim + `(s,t)` at each
prim texel (`receiverAt`, the same code that samples the surface **silhouette**). Reading the **normal** is
the identical move on a normal atlas — `frame_origin + (s,t)·frame_size`, `texelFetch` — an **atlas lookup
indexed by frame, NOT a world-coord read of a `textile_slot` composite**, so it's in-family and zoom-safe by
construction ([forks.md#f3](forks.md#f3), user). The atlas normal is **baked already in the world frame** —
the pitch is applied at `bin/art` ingest (DSL orientation: ground → up, thing → horizontal), so the bake
just `texelFetch`es it and dots — **no runtime pitch, no runtime orientation flag** ([forks.md#f7](forks.md#f7)).
Cost: the world angle is **bake-committed** (re-ingest to change; `__tilt` live desyncs). Ground tiles are
prims with a generic white texture (same path). Optional paired win: co-pack albedo/normal/surface into one
atlas so a **quadrant shift** of the frame grabs any channel ([forks.md#f6](forks.md#f6)).

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
