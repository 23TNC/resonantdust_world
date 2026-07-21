# Forks — webgl-engine

_Decisions with live alternatives. Resolve in place; record the pick + why._

---

## F1 · The new folder / component name

- **`client/webgl` (chosen default).** Parallels `client/pixijs` (each named for its renderer); says exactly
  what it is (raw WebGL2).
- `client/engine` — emphasises "we own the engine now", but vaguer about the renderer.
- `client/native` — "native WebGL", but overloaded (native = desktop elsewhere).

**Pick:** `client/webgl` unless you prefer otherwise. _(pending)_

## F2 · How much to copy vs rewrite

- **Copy pure logic ~verbatim; rewrite only the render seams (chosen).** The DOM UI, atlas/LOD, DSL, sync,
  login, WASM seam, and the math are renderer-agnostic — copy them and fix imports. Rewrite ONLY what touches
  Pixi's `Mesh`/`Geometry`/`Shader`/`Texture`/`RenderTexture` (the SquareCache render path + the shader
  harness + Sprite/Graphics/Text). Keeps the diff small and the logic proven.
- **Rewrite everything fresh.** Cleaner in theory, but throws away working, tested code (the atlas packer,
  the resolver, the sync clock) for no renderer reason. Rejected.

**Pick:** copy logic, rewrite seams. _(pending)_

## F3 · State model — explicit vs tracked

- **Small tracked-state cache (chosen).** Track the currently-bound program/VAO/target + blend/scissor/
  viewport, skip redundant GL calls. This is what Pixi did; owning it end-to-end is what removes the interop
  corruption. Start minimal (correctness first), optimise later.
- **Fully explicit (set all state every draw).** Simplest + always correct, but wasteful. Fine as the W2
  starting point; add the cache once parity is close.

**Pick:** explicit first (W2), add a minimal tracked cache before W6. _(pending)_

## F4 · The convenience primitives — replace now or lean on a stopgap

- **Replace immediately (chosen).** `Text` → DOM overlays (UI is already DOM); `Sprite` → a quad; `Graphics`
  → a small line/rect/circle helper on the engine (+ CSS for panel chrome). No third-party primitive lib.
- **Pull a tiny 2D-vector lib** for Graphics-like shapes. Extra dependency for shapes we mostly don't need.

**Pick:** replace immediately, no new deps. _(pending)_

## F5 · Cutover — parallel until parity

- **Parallel clients until parity, then retire pixijs (chosen).** `client/webgl` develops alongside a
  working `client/pixijs`; only when W6 parity is verified does webgl become primary and pixijs get archived
  out-of-repo. Never a broken game.
- **Hard switch early.** Faster but risks a long broken window. Rejected — the whole point of the separate
  folder is to avoid that.

**Pick:** parallel until parity. _(pending)_
