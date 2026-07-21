# Completed — shadow-projection

_Done + verified. Items move here from [`todo.md`](todo.md) (append-only history; authoritative for what's
done). Nothing yet — the stream opened 2026-07-21._

## P0–P2 · GPU-instanced projected 5-triangle fan — 2026-07-21

The core strategy shift, landed in one slice: the `ShadowCaster` rewritten from the per-pixel analytic
trapezoid to the sandbox's projected-silhouette fan, **built on the GPU**. One instance per (light, caster);
the vertex shader (`shadowCaster.ts` `CAST_VERT`) reads the instance's caster (`Ax,Ay,W,H,θ,dA,dB`) + light
(`Lx,Ly,Lz`), runs `cornersWith` + per-corner `proj` (each corner projects by its own z, TMAX 8), and emits
the 15 fan vertices (**T1** body + **T2/T3/T4/T5** ±depth) via a `gl_VertexID`→role table. Instance data via
**instance vertex attributes** ([F3](forks.md#f3) sub-choice (a)); the CPU keeps only the cheap in-range
(light, caster) **pairing** — no per-frame CPU geometry. Engine tweak: `Geometry.instanceCount` is now mutable
so a per-frame cast varies the pair count after `update()`-ing the instance buffers.

**Verified in-browser at `?focus=100,50`:** shadows render as GPU triangle fans radiating from the conifers
away from the lights, over the W4h textured world, world-stuck under pan/zoom, zero console errors — the
strategy shift (analytic per-pixel → GPU-instanced geometry) is proven, and it is `caster-lut` C5 /
[webgl-engine W7](../webgl-engine/todo.md) on the owned engine. This slice ships the **E/W regime** (all
current cold things are single-facing → E/W), a **constant θ** (65°), a **rough width-fraction depth**
(`W·0.22`), and **solid semi-transparent** triangles. The remaining phases refine it: P3 depth-from-presence,
P4 the alpha-mask fragment (silhouette-shaped), P5 the N/S facing regime, plus a blending pass (per-light
combine / the bitfield the `shadows` stream consumes).

## P3 · Base depth from the sprite silhouette — 2026-07-21

Replaced the rough width-fraction base spread with the sandbox's auto rule. The `ShadowCaster` takes the
resolver, resolves each caster's `surface` frame, reads it back once (FBO + `readPixels` of the atlas region),
thresholds `B > 0.5` into a presence bitmap, and computes `depth = ½·(avg opaque HEIGHT of the half's columns
/ TS)·H` (left→`dA`, right→`dB`), **cached per stem** (not per frame). Falls back to the rough default until a
stem's surface LOD resolves. Verified: shadows render with silhouette-derived base spread, zero console errors.

## P4 · Alpha-masked silhouette shadows — 2026-07-21

The fragment samples each caster's `surface.B` coverage through per-role sprite UVs (E/W bottom-edge:
`TL(0,0) TR(1,0) BL±/BR±(0/1,1) BC(.5,1)`) and **discards outside the silhouette** — a shadow reads as the
caster's SHAPE (a projected conifer), not a solid polygon. The vertex assigns the UV per role + passes the
surface atlas sub-frame (`aFrame`, flat); the caster feeds its `uvRect` per instance + binds the shared
surface page (all 64px surfaces share one page; `zw=0` → solid until resolved). **Verified in-browser at
`?focus=100,50`:** shadows render as coloured **conifer silhouettes** projected away from the lights over the
textured forest, world-stuck, zero console errors — the sandbox's projected-silhouette model, fully on the GPU.

_Stream core delivered: the 5-triangle projected-silhouette fan, GPU-instanced, silhouette-masked, matching
the sandbox. Remaining are non-core refinements (P5 below)._
