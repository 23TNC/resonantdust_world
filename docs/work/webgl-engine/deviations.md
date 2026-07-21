
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

## D-2 · W4f shadows: analytic single-pass cast, not the screen-hot→world-cold bitfield — 2026-07-21

The user pivoted W4f from "port shadowCast + the debug spikes" to "get the first lights + shadows off the
prims online (basic functionality, geo tier, textures later)." Rather than port the pixijs
`shadowCast`/`shadowCastShaders` machinery 1:1 — the `docs/work/shadows/` design's **screen-hot RGB lanes →
world-cold toroidal bitfield**, ping-pong merge, 4-copy toroidal pack, round-robin, caster-lut data textures
— the webgl cut casts shadows **analytically in ONE screen-space fullscreen pass** (`shadowCaster.ts`): per
fragment, world-position → loop lights × in-range casters → point-in-projected-trapezoid → sum the covering
lights' colours.

**Why:** on the ES 3.00 owned engine this is the design's *endpoint* (the GPU cast, caster-lut C5), and for
"basic functionality" it needs zero of the persistence machinery — recasting every frame at the current
camera is inherently world-stuck + zoom-correct with no staleness (the shadows README's own `shadow-hot`
insight), so the world-cold bitfield + round-robin are pure scaling optimisations, deferrable. It's also far
less code to land + verify. Deliberately dropped for now: the world-space `shadow-cold` (so shadows are NOT
`/overlayRT`-inspectable yet — they display directly), per-light **bit** packing (colours are summed per-light
instead), and the round-robin. **Cost:** the per-pixel analytic loop runs ~49fps (from ~56 unlit) at 6 lights.

**How to apply:** when the count/cost grows (toward the 24-light goal), add the world-cold persistence +
round-robin from the shadows-stream design ON TOP — the analytic cast becomes the per-frame `shadow-hot`
generator that packs into `shadow-cold`. The trapezoid projection + light/caster model carry over unchanged.
