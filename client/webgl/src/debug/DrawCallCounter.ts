//! DrawCallCounter — STUB for W3. In the pixijs client this monkey-patched the
//! Pixi GL context to tally draw calls per frame for the debug HUD. The webgl
//! client owns its renderer, so the real counter will be a plain increment inside
//! `Renderer.draw` surfaced here — wired in [webgl-engine W4](../../../docs/work/webgl-engine/todo.md)
//! when the viewport starts drawing. Until then this reports 0 so the HUD row is
//! present but inert.

export class DrawCallCounter {
  /** No-op until the engine renderer exposes a per-frame draw tally (W4). */
  patch(_renderer?: unknown): void {}

  /** Draw calls since the last read (always 0 in the stub). */
  readAndReset(): number {
    return 0;
  }
}
