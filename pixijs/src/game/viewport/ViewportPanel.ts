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
  private readonly unsubResize: () => void;

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
    this.viewport = new Viewport(ctx.textures);
    this.content.addChild(this.viewport);
    this.sizeViewport();
    // PixiPanel syncs `content` bounds first on each rect change (it subscribed in its
    // own constructor, before this one), so the viewport reads the fresh body size.
    this.unsubResize = this.onRectChange(() => this.sizeViewport());
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
    this.viewport.destroy();
    super.destroy();
  }
}
