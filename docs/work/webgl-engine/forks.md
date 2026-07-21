# Forks — webgl-engine

_Decisions with live alternatives. Resolve in place; record the pick + why._

---

## F1 · The new folder / component name

- **`client/webgl` (chosen default).** Parallels `client/pixijs` (each named for its renderer); says exactly
  what it is (raw WebGL2).
- `client/engine` — emphasises "we own the engine now", but vaguer about the renderer.
- `client/native` — "native WebGL", but overloaded (native = desktop elsewhere).

**Pick:** `client/webgl`. ✓ **confirmed** — built + running on 5174 (W1/W2).

## F2 · How much to copy vs rewrite

- **Copy pure logic ~verbatim; rewrite only the render seams (chosen).** The DOM UI, atlas/LOD, DSL, sync,
  login, WASM seam, and the math are renderer-agnostic — copy them and fix imports. Rewrite ONLY what touches
  Pixi's `Mesh`/`Geometry`/`Shader`/`Texture`/`RenderTexture` (the SquareCache render path + the shader
  harness + Sprite/Graphics/Text). Keeps the diff small and the logic proven.
- **Rewrite everything fresh.** Cleaner in theory, but throws away working, tested code (the atlas packer,
  the resolver, the sync clock) for no renderer reason. Rejected.

**Pick:** copy logic, rewrite seams. ✓ **confirmed** — W3 survey: 34/62 files Pixi-free (copy verbatim); 28 touch Pixi, mostly one shallow import.

## F3 · State model — explicit vs tracked

- **Small tracked-state cache (chosen).** Track the currently-bound program/VAO/target + blend/scissor/
  viewport, skip redundant GL calls. This is what Pixi did; owning it end-to-end is what removes the interop
  corruption. Start minimal (correctness first), optimise later.
- **Fully explicit (set all state every draw).** Simplest + always correct, but wasteful. Fine as the W2
  starting point; add the cache once parity is close.

**Pick:** explicit first (W2), add a minimal tracked cache before W6. ✓ **confirmed** — Renderer sets state per-draw today; cache deferred.

## F4 · The convenience primitives — replace now or lean on a stopgap

- **Replace immediately (chosen).** `Text` → DOM overlays (UI is already DOM); `Sprite` → a quad; `Graphics`
  → a small line/rect/circle helper on the engine (+ CSS for panel chrome). No third-party primitive lib.
- **Pull a tiny 2D-vector lib** for Graphics-like shapes. Extra dependency for shapes we mostly don't need.

**Pick:** replace immediately, no new deps. ✓ **confirmed** — see F6 for the shell shape this implies.

## F5 · Cutover — parallel until parity

- **Parallel clients until parity, then retire pixijs (chosen).** `client/webgl` develops alongside a
  working `client/pixijs`; only when W6 parity is verified does webgl become primary and pixijs get archived
  out-of-repo. Never a broken game.
- **Hard switch early.** Faster but risks a long broken window. Rejected — the whole point of the separate
  folder is to avoid that.

**Pick:** parallel until parity. ✓ **confirmed** — pixijs dev server can be stopped now (user, 2026-07-20); code stays in-repo until W6 cutover.

## F6 · The app shell — global scene-graph vs DOM-composited (W3)

The pixijs client renders through **two** Pixi surfaces: the **viewport** (WebGL shaders → SquareCache) and
**panel chrome + cards** (`Graphics`/`Text`/`BitmapFont` via `LayoutNode`, canvas-overlaid on the DOM panel
bodies). Everything else — login form, panel bodies, taskbars, popups — is already DOM. So the shell question
is what replaces Pixi's `Application`/`stage`/`Container`/`Ticker`.

- **DOM-composited, no global scene-graph (chosen).** The shell (`app/App` + `app/Ticker`) owns only a
  RAF ticker (deltaMS), a resize dispatch, the SceneManager, and the GameContext — **no global canvas, no
  Container tree.** The one WebGL surface (the viewport) owns **its own canvas** inside its DOM panel, driven
  by a `Renderer` instance. `Scene` drops `root: Container`; scenes self-mount/unmount their DOM + viewport
  canvas in `onEnter`/`onExit`. Panel **chrome** (outlines, resize grips, drag affordance) becomes **CSS**
  (borders/box-shadow/`::before`) on the existing DOM panel — no canvas draw. **Cards** (the only genuine 2D
  vector/text content) defer to a later sub-phase: either a tiny engine `Graphics` helper (F4) or DOM/CSS;
  decide when we reach them (none render at login/world-boot).
- **Rebuild a Pixi-like scene-graph** (our own `Container`/`Sprite`/`Text` tree, one big canvas). Faithful to
  the current structure but re-implements a retained-mode 2D engine we don't need — the app is DOM-first and
  the viewport is the only real GPU surface. Rejected.

**Pick:** DOM-composited, no global scene-graph. Viewport self-canvases; chrome → CSS; cards deferred.
**Consequence for the port:** `Scene`/`SceneManager`/`GameContext`/`main` get rewritten (not copied);
`LayoutNode`/`PixiPanel` collapse into CSS on `DomPanel`; `assets/fonts` drops `BitmapFont.install` (keep the
`FontFace` registration). ✓ decided 2026-07-20.
