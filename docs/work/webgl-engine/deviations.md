
## D-1 · W3 login-boot ships with five render-layer stubs — 2026-07-20

The W3 milestone (login DOM form + WASM init + gateway connect) needs none of the render layer, so five
files are **stubs** on `client/webgl`, each replaced when its subsystem ports in W4:
- `scenes/world/WorldScene.ts` — placeholder "Logged in." DOM; the real viewport-hosting scene is W4.
- `textures/TextureResolver.ts` — no-GPU: records the `/textures` root + reports empty `lodStats`; the
  atlas-backed resolver (shares the viewport GL context) is W4. `textures/index.ts` drops the
  `TextureAtlas`/`LodPool`/`MaxRectsPacker` re-exports until then.
- `debug/DrawCallCounter.ts` — reports 0; wires to `Renderer.draw`'s tally in W4.
- `game/layout/LayoutNode.ts` — a minimal structural `interface` (parent-linked) so `PanelManager`'s
  node-tracking type-checks; the Pixi `Container` chrome collapses to CSS ([F6](forks.md#f6)) with the cards.
- `game/panels/titlebar/VideoPanel.ts` — frame-cap wires to our `Ticker.maxFPS` (live); render-scale is
  recorded but not applied (no global renderer yet) — pushed to the viewport DPR in W4.

**Why:** keeps W3 a small, verifiable slice (the whole non-render client) without dragging the render port
forward. **How to apply:** each stub carries a header naming its W4 replacement; grep `STUB` under
`client/webgl/src`.
