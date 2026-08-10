# Component — `client/webgl`

_Path: `client/webgl`. Deploys as: the browser client bundle (TS + a bespoke WebGL2 engine).
Last updated: 2026-07-28 — this folder moved from `client/pixijs`, which is DELETED (webgl
superseded it; git holds the pixijs history)._

Renders the world from the shard's `entity_state` + cold tables (relayed by edge, via
`client/core` over wasm): terrain + things (WorldBridge) and pawns (MoverLayer), composited from
the SquareCache G-buffer. **The client renders UNLIT** — see [`current/`](current/).

- **[`design/`](design/)** — [`rendering-platform.md`](design/rendering-platform.md) (WebGL2,
  GLSL ES 3.00, the bespoke `gl/` engine); [`de-lighting.md`](design/de-lighting.md) (albedo
  extraction — art-pipeline-facing); [`input-model.md`](design/input-model.md) (right = select,
  left = the context pie menu); [`panel-layout.md`](design/panel-layout.md) (the 58×33 cell grid
  — every panel, both taskbars, title bars and the text scale in cells of one grid);
  [`live-edit.md`](design/live-edit.md) (the `/edit` inspector: preview + four tabs over ONE eval
  snapshot).
- **[`current/`](current/)** — what the renderer does today, and where a lighting system attaches.
- `intent/`, `plan/` — lazy.

The living execution state for renderer work is under `docs/work/` (lighting/shadow/material
streams); this folder holds durable design + intent only. NOTE: parts of these docs predate the
pixijs→webgl port — cited `webgl/src/` paths exist where the port kept the name, but verify
against code before planning from one.

> `client.md` lives at [`client/core/intent/`](../core/intent/client.md) (moved 2026-07-14 — it
> describes the headless Rust client end to end).
