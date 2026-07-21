//! Scene — a top-level app state (login, world). Unlike the pixijs client there's
//! no `root: Container`: the shell has no global scene-graph
//! ([webgl-engine F6](../../../docs/work/webgl-engine/forks.md)), so a scene
//! **self-mounts** its DOM (and, for the world scene, its viewport `<canvas>`) in
//! `onEnter` and tears it down in `onExit`. Lifecycle is driven by
//! {@link SceneManager}; subclasses override the hooks they need.

import type { GameContext } from "../GameContext";

export abstract class Scene {
  width = 0;
  height = 0;

  /** Called by SceneManager on entry and on window resize. */
  resize(width: number, height: number): void {
    this.width = width;
    this.height = height;
    this.onResize(width, height);
  }

  onEnter(_ctx: GameContext): void | Promise<void> {}

  onExit(): void | Promise<void> {}

  onResize(_width: number, _height: number): void {}

  /** deltaMS = real milliseconds since the last frame ({@link Ticker}); the
   *  canonical timebase for all scene logic. */
  update(_deltaMS: number): void {}
}
