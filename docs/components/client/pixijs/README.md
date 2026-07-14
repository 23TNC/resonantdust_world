# Component — `client/pixijs`

_Path: `client/pixijs`. Deploys as: the browser client bundle (TS + pixi.js). Last updated: 2026-07-14._

Renders the world from the shard's `state` + `cold` (relayed by edge, via `client/core` over
wasm): terrain + trees (WorldBridge) and pawns (MoverLayer), with the lighting model.

- **[`design/`](design/)** — [`lighting.md`](design/lighting.md),
  [`de-lighting.md`](design/de-lighting.md) (albedo extraction).
- **[`intent/`](intent/)** — [`client.md`](intent/client.md) (the client's shape/role),
  [`zones-to-screen.md`](intent/zones-to-screen.md) (the wiring: biomes+trees → browser).
- `current/`, `plan/` — lazy. (Rendering pipeline detail; `client.md` may split toward
  `client/core` when that component is worked.)
