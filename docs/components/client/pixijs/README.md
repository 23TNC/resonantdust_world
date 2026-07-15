# Component — `client/pixijs`

_Path: `client/pixijs`. Deploys as: the browser client bundle (TS + pixi.js). Last updated: 2026-07-14._

Renders the world from the shard's `state` + `cold` (relayed by edge, via `client/core` over
wasm): terrain + trees (WorldBridge) and pawns (MoverLayer), with the lighting model.

- **[`design/`](design/)** — [`lighting.md`](design/lighting.md),
  [`de-lighting.md`](design/de-lighting.md) (albedo extraction).
- **[`intent/`](intent/)** — [`zones-to-screen.md`](intent/zones-to-screen.md) (the wiring:
  biomes+trees → browser).
- `current/`, `plan/` — lazy.

> `client.md` **moved to [`client/core/intent/`](../core/intent/client.md)** (2026-07-14). It was
> filed here but described the **headless Rust client** end to end — host API (`client/core/src/api.rs`),
> login flow, config, the `headless` binary — mentioning pixijs only as a future *consumer* of it.
> No split was needed; the whole file was misfiled.
