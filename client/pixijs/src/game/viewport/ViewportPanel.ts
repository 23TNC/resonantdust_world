//! The world viewport panel — a {@link PixiPanel} whose body hosts the {@link Viewport}
//! (albedo-map renderer). The DOM shell owns drag / resize / minimize / taskbar; this
//! wrapper sizes the viewport to the body rect and drives its per-frame tick.
//!
//! Geometry + chrome flags come from the `worldViewport` entry in
//! `content/panels/defaults.json` (title bar hidden, height-mode full, masked).

import type { Renderer } from "pixi.js";
import { PixiPanel } from "../../ui/dom/PixiPanel";
import type { LayoutNode } from "../layout/LayoutNode";
import type { GameContext } from "../../GameContext";
import { panelTitle } from "../panels/panelStrings";
import type { RtChannel } from "../panels/rt/RtPanel";
import { Viewport } from "./Viewport";

export class ViewportPanel extends PixiPanel {
  private readonly viewport: Viewport;
  private readonly renderer: Renderer;
  private readonly ctx: GameContext;
  private readonly unsubResize: () => void;
  /** Window pointer listener driving the viewport's cursor light. */
  private readonly onPointerMove: (e: PointerEvent) => void;

  constructor(ctx: GameContext, parent: LayoutNode) {
    super({
      parent,
      title: panelTitle("gameViewPanel"),
      storageKey: "worldViewport",
      minWidth: 240,
      minHeight: 200,
      taskbar: ctx.taskbar,
      pinned: true,
      uiEditMode: ctx.uiEditMode,
    });
    this.renderer = ctx.app.renderer;
    this.ctx = ctx;
    this.viewport = new Viewport(ctx.textureResolver);
    this.content.addChild(this.viewport);
    this.sizeViewport();
    // PixiPanel syncs `content` bounds first on each rect change (it subscribed in its
    // own constructor, before this one), so the viewport reads the fresh body size.
    this.unsubResize = this.onRectChange(() => this.sizeViewport());

    // Drive the cursor light: map the pointer (when over the body) to a world point and
    // hand it to the rig; clear it when the pointer leaves. A window listener (not a body
    // one) so the light also clears when the pointer moves off-panel onto another window.
    this.onPointerMove = (e: PointerEvent): void => {
      const r = this.bodyRect;
      const lx = e.clientX - r.left;
      const ly = e.clientY - r.top;
      if (lx >= 0 && lx < r.width && ly >= 0 && ly < r.height) {
        const w = this.viewport.screenToWorld(lx, ly);
        this.viewport.lights.setCursorWorld(w.x, w.y);
        this.ctx.debugPanel?.setCursorCoords(w);
      } else {
        this.viewport.lights.setCursorWorld(null);
        this.ctx.debugPanel?.setCursorCoords(null);
      }
    };
    window.addEventListener("pointermove", this.onPointerMove);
  }

  private sizeViewport(): void {
    this.viewport.setBounds(0, 0, this.content.width, this.content.height);
  }

  /** The underlying viewport renderer — the world scene's bridge feeds it tiles
   *  and drives its anchor. */
  get view(): Viewport {
    return this.viewport;
  }

  /** This viewport's render-texture channels for the `/showRT` preview. */
  renderTextures(): RtChannel[] {
    return this.viewport.renderTextures();
  }

  /** This viewport's display aspect ratio (width / height). */
  aspect(): number {
    return this.viewport.aspect();
  }

  /** Drive the viewport's per-frame bake + display rebuild. The world scene calls this
   *  every frame. Re-syncs size first so a missed rect-change event can't strand it. */
  tick(): void {
    this.sizeViewport();
    this.viewport.tick(this.renderer);
  }

  override destroy(): void {
    this.unsubResize();
    window.removeEventListener("pointermove", this.onPointerMove);
    this.viewport.destroy();
    super.destroy();
  }
}
