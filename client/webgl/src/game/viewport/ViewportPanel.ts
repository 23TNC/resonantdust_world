//! The world viewport panel — a {@link DomPanel} whose body hosts the {@link Viewport}'s
//! own `<canvas>` (F6: no shared Pixi canvas; the viewport self-canvases). The DOM shell
//! owns drag / resize / minimize / taskbar; this wrapper sizes the viewport to the body
//! rect and drives its per-frame tick. Geometry + chrome flags come from the
//! `worldViewport` entry in `content/panels/defaults.json` (title bar hidden, full-height,
//! masked), so it fills the screen.

import { DomPanel } from "../../ui/dom/DomPanel";
import type { GameContext } from "../../GameContext";
import { panelTitle } from "../panels/panelStrings";
import { Viewport } from "./Viewport";

export class ViewportPanel extends DomPanel {
  private readonly viewport = new Viewport();
  private readonly unsubResize: () => void;

  constructor(ctx: GameContext) {
    super({
      title: panelTitle("gameViewPanel"),
      storageKey: "worldViewport",
      minWidth: 240,
      minHeight: 200,
      taskbar: ctx.taskbar,
      pinned: true,
      uiEditMode: ctx.uiEditMode,
    });
    const holder = document.createElement("div");
    holder.style.cssText = "position:absolute;inset:0;overflow:hidden;";
    holder.appendChild(this.viewport.canvas);
    this.setBody(holder);
    this.sizeViewport();
    this.unsubResize = this.onRectChange(() => this.sizeViewport());
  }

  private sizeViewport(): void {
    const r = this.bodyRect;
    this.viewport.setBounds(r.width, r.height);
  }

  /** The underlying viewport renderer — the world scene feeds it the camera + (later) tiles. */
  get view(): Viewport {
    return this.viewport;
  }

  /** The viewport canvas — input listeners (drag-pan, scroll-zoom) attach here. */
  get canvas(): HTMLCanvasElement {
    return this.viewport.canvas;
  }

  /** Drive the per-frame render. The world scene calls this every frame; re-syncs size
   *  first so a missed rect-change event can't strand it. */
  tick(): void {
    this.sizeViewport();
    this.viewport.tick();
  }

  override destroy(): void {
    this.unsubResize();
    this.viewport.destroy();
    super.destroy();
  }
}
