//! SceneManager — owns the current {@link Scene} and the serialized transitions
//! between scenes. Same contract as the pixijs client, minus Pixi: it drives
//! `update(deltaMS)` off the {@link App} ticker and `resize` off the app's
//! window-resize dispatch, and there's no `stage.addChild` — a scene mounts its
//! own DOM/canvas in `onEnter` and removes it in `onExit`. Transitions chain so a
//! rapid change can't interleave two scenes' enter/exit.

import type { App } from "../app/App";
import type { GameContext } from "../GameContext";
import { Scene } from "./Scene";

export class SceneManager {
  private current: Scene | null = null;
  private context: GameContext | null = null;
  private transitionChain: Promise<void> = Promise.resolve();
  private disposed = false;
  private readonly tick: (deltaMS: number) => void;
  private readonly onResize: () => void;

  constructor(private readonly app: App) {
    this.tick = (deltaMS) => {
      this.current?.update(deltaMS);
    };
    this.onResize = () => {
      this.current?.resize(this.app.width, this.app.height);
    };
    app.ticker.add(this.tick);
    app.onResize(this.onResize);
  }

  setContext(context: GameContext): void {
    if (this.context) {
      throw new Error("SceneManager: context already set");
    }
    this.context = context;
  }

  change(scene: Scene): Promise<void> {
    if (this.disposed) {
      return Promise.reject(new Error("SceneManager: disposed"));
    }
    this.transitionChain = this.transitionChain
      .catch(() => undefined)
      .then(() => this.performChange(scene));
    return this.transitionChain;
  }

  async dispose(): Promise<void> {
    if (this.disposed) return;
    this.disposed = true;
    await this.transitionChain.catch(() => undefined);
    if (this.current) {
      const previous = this.current;
      this.current = null;
      await previous.onExit();
    }
    this.app.ticker.remove(this.tick);
    this.app.offResize(this.onResize);
  }

  private async performChange(scene: Scene): Promise<void> {
    if (this.disposed) return;
    if (!this.context) {
      throw new Error("SceneManager.change called before setContext");
    }

    const previous = this.current;
    if (previous) {
      this.current = null;
      await previous.onExit();
    }
    if (this.disposed) return;

    await scene.onEnter(this.context);
    if (this.disposed) {
      await scene.onExit();
      return;
    }

    scene.resize(this.app.width, this.app.height);
    this.current = scene;
  }
}
