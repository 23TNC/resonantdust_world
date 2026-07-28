# Component — `client/webgl`

_Path: `client/webgl`. Deploys as: the browser client bundle (TS + a bespoke WebGL2 engine).
Last updated: 2026-07-28 — this folder moved from `client/pixijs`, which is DELETED (webgl
superseded it; git holds the pixijs history)._

Renders the world from the shard's `entity_state` + cold tables (relayed by edge, via
`client/core` over wasm): terrain + things (WorldBridge), pawns (MoverLayer), and the lighting
model (SquareCache G-buffer channels + baked lightmaps).

- **[`design/`](design/)** — [`rendering-platform.md`](design/rendering-platform.md) (WebGL2,
  GLSL ES 3.00, the bespoke `gl/` engine); [`lighting.md`](design/lighting.md),
  [`shadows.md`](design/shadows.md), [`de-lighting.md`](design/de-lighting.md) (albedo
  extraction — art-pipeline-facing).
- **[`intent/`](intent/)** — [`tiered-lighting.md`](intent/tiered-lighting.md),
  [`shadows.md`](intent/shadows.md) (the target lighting/shadow models the rebuilds steer by).
- `current/`, `plan/` — lazy.

The living execution state for renderer work is under `docs/work/` (lighting/shadow/material
streams); this folder holds durable design + intent only. NOTE: parts of these docs predate the
pixijs→webgl port — cited `webgl/src/` paths exist where the port kept the name, but verify
against code before planning from one.

> `client.md` lives at [`client/core/intent/`](../core/intent/client.md) (moved 2026-07-14 — it
> describes the headless Rust client end to end).
