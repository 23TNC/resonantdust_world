# Work — prim-batching (one mesh per atlas page, not one per prim)

_Opened 2026-07-18. A `client/pixijs` rendering-perf stream: collapse the G-buffer **bake** from one
draw call per prim to one per atlas **page**. Touches [`viewport`](../../components/client/pixijs)
(`SquareCache` + the four bake shaders + `TextureResolver`/`TextureAtlas`). No server/protocol change._

## What & why

The viewport is a deferred G-buffer: prims bake into toroidal render-texture channels (albedo /
material / surface / depth / normal), then one lighting mesh samples them. The **sampling** side is
already a handful of draws — the win the RT design was for. The **bake** side is not: today
[`SquareCache.bakeSquare`](../../../client/pixijs/src/game/viewport/SquareCache.ts) emits **one `Mesh`
per prim per channel** — `materialNode` / `surfaceNode` / `depthNode` / `normalNode` each build a
pooled per-prim `Mesh` with its per-instance data in **shader uniforms**; only geo-tier / untextured
prims fall to the batched `spriteNode`. In a loaded scene (textures resolved → the material path) that
is ~4–5 mesh draws for **every** tile and thing, per bake. On a pan that re-bakes many squares it is
thousands of draw calls.

Measured (2026-07-18, at rest): `tick()`'s draws were **100 % the shadow pass** with both bakes at 0 —
i.e. the bake is correctly *amortized* (static prims bake once, then we sample). So this work is about
the **pan / zoom / load** draw-call spikes (when squares go dirty and re-bake), not the idle frame.
The [shadow pass](../../../client/pixijs/src/game/viewport/shadowPass.ts) is the *other* per-prim-mesh
site and the idle-frame cost — **explicitly deferred** to a later stream.

## The fix

**One mesh per atlas page.** A resolved `Texture` exposes `.source` (the atlas page's backing GL
texture — the batch key) and `.frame` (its sub-rect → per-prim UV). So bucket a channel's prims by
`source` and emit **one batched `Mesh` per (channel, page)** whose geometry is a quad per prim, with the
per-instance data that is currently **uniforms** moved to **vertex attributes** (`aUVFrame`,
`aWorldRect`, `aTint`, `aSeed`, `aFlip`, + material channel params). One texture bound per draw; one
draw per page. **The hard part is the shader-attribute refactor, not the batching.**

## Scope decision (see [`forks.md`](forks.md) F1)

**(A) per-square page-batch** — batch _within_ `bakeSquare` (group the square's prims by page). Keeps
the toroidal scratch/blit + amortization **untouched**; draws/bake = `channels × pages_in_square`
(~1–3), not `channels × prims`. **Chosen** — it kills the per-prim draws, keeps the proven design, and
de-risks the shader refactor. **(B) global persistent per-page mesh** (add/remove by dirty tile — the
lowest draw count) fights the per-square wrap-apron and risks breaking amortization; deferred, revisit
only if per-square page counts still bite.

## Invariants (must not regress)

- **Pixel-identical** G-buffer output vs the per-prim path (verify per channel).
- **Amortization** — a still frame bakes nothing; only dirty squares re-bake.
- **Wrap-apron** — the toroidal edge blit stays correct (batching is inside the scratch render).
- **west `flipX`**, **LOD page migration**, **z-order** (safe: opaque G-buffer, `zdepth`-resolved —
  atlas interleaving doesn't matter; this is *why* the deferred design batches cleanly).

## State

Phased build in [`todo.md`](todo.md). Design/measurement notes accumulate in `notes/` as we go.
