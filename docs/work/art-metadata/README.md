# Work — art-metadata (the per-sprite `meta.json` sidecar)

_Opened 2026-07-18. A `dev`/`bin/art` stream: the map pipeline emits a **metadata sidecar**
(`meta.json`) per variant leaf, beside the pixel maps, for the art/texture metadata the runtime needs
that isn't a pixel map. First consumers: the packed-channel **tints** and the shadow **outline** — the
outline being the missing input the [tiered-lighting](../../intent) port depends on (see
`../resonantdust/docs/tiered_lighting.md`)._

## What & why

`bin/art maps` bakes the pixel maps (`albedo`/`normal`/`surface`/`layers`/…). Two things the runtime
needs are **not** pixels:
- **packed-channel tints** — `split_layers` decomposes the albedo into `layers.png` (RGB material
  weights) + a residual, so the client reconstructs `albedo + Σ layersᵢ·tintᵢ`. The per-channel base
  colours (`tintᵢ`) were computed but only *logged* — the client needs them to reconstruct + re-tint.
- **the shadow outline** — the tiered-lighting scatter projects a caster's **vector silhouette**
  (boundary polygons + earcut triangulation) through each light. The new game bakes a *raster*
  silhouette (surface `A`) but never vectorizes it — the [gap that gates the lighting port](../../work).

Both (and future metadata) live in one extensible, **read-merge-write** sidecar so each generator
contributes its keys independently — `bin/lib/meta.py` (`meta.update(map_path, key=…)`), co-located as
`meta.json` and folded into the content manifest like `atlas.json`.

## State

Phased in [`todo.md`](todo.md). **P1 (sidecar + tints) DONE + verified**; **P2 (outline generation)**
is the real work — port the old game's `alpha → marching-squares → Visvalingam simplify → earcut`
(`../resonantdust/shared/geometry`) into a Python map generator writing `outline` into the sidecar.
