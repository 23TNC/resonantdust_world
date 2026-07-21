//! App — the application shell, replacing Pixi's `Application`. Unlike Pixi it
//! owns **no global canvas and no scene-graph**: the client is DOM-first (login
//! form, panel bodies, taskbars are all DOM) and the one WebGL surface — the
//! viewport — owns its own `<canvas>` inside its DOM panel (see
//! [webgl-engine F6](../../../../docs/work/webgl-engine/forks.md)). So the shell
//! carries only what's genuinely app-global: the {@link Ticker}, a window-resize
//! dispatch, and the `#app` mount host. Scenes self-mount their DOM in `onEnter`.

import { Ticker } from "./Ticker";

export class App {
  readonly ticker = new Ticker();
  /** The `#app` element scenes + panels mount into. */
  readonly mount: HTMLElement;
  private readonly resizeFns = new Set<() => void>();

  constructor(mount: HTMLElement) {
    this.mount = mount;
    window.addEventListener("resize", this.onWindowResize);
  }

  /** Logical (CSS-pixel) viewport size — the DPR scaling lives in the renderer. */
  get width(): number {
    return window.innerWidth;
  }
  get height(): number {
    return window.innerHeight;
  }

  onResize(fn: () => void): void {
    this.resizeFns.add(fn);
  }
  offResize(fn: () => void): void {
    this.resizeFns.delete(fn);
  }

  /** Start the frame loop. Call once after the initial scene is installed. */
  start(): void {
    this.ticker.start();
  }

  private readonly onWindowResize = (): void => {
    for (const fn of this.resizeFns) fn();
  };
}
