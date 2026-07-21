//! WorldScene — the real world scene. Hosts the {@link ViewportPanel} (its own canvas + engine
//! Renderer) and wires the {@link WorldBridge} (subscribed-zone cold tiles/things → the cold
//! SquareCache, camera anchor → client subscription) + the {@link MoverLayer} (pawns → the warm
//! cache, composited over cold). Drag pans, scroll zooms (about the cursor), both routed THROUGH
//! the bridge so zone subscriptions follow the view. `?focus=x,y` frames the initial anchor;
//! `?grid` overlays the debug grid. Chat, RT panels, and the debug `/commands` are later slices (W4f).

import { Scene } from "../Scene";
import type { GameContext } from "../../GameContext";
import { ViewportPanel } from "../../game/viewport/ViewportPanel";
import { WorldBridge } from "../../game/world/WorldBridge";
import { MoverLayer } from "../../game/world/MoverLayer";
import { parseUrl, argFlag } from "../../debug/urlParams";

/** Wheel deltaY per zoom octave: factor = 2^(-deltaY/this), applied per event (continuous). */
const WHEEL_OCTAVE = 500;

export class WorldScene extends Scene {
  private panel!: ViewportPanel;
  private bridge!: WorldBridge;
  private moverLayer!: MoverLayer;
  private dragId: number | null = null;
  private lastClientX = 0;
  private lastClientY = 0;

  onEnter(ctx: GameContext): void {
    this.panel = new ViewportPanel(ctx);
    this.panel.open();

    // The client zone stream → viewport tiles, and the camera anchor → client subscription.
    this.bridge = new WorldBridge(ctx.client, ctx.content, this.panel.view, this.panel.view.white, ctx.textureResolver);
    // Pawns (the wolves): synced from the tick pipeline's mobile entities into the viewport's WARM cache.
    this.moverLayer = new MoverLayer(ctx.client, ctx.content, this.panel.view);

    // URL: initial anchor (x/y/focus tile coords) + the debug grid.
    let tileX = 0;
    let tileY = 0;
    for (const cmd of parseUrl().commands) {
      if (cmd.name === "focus") {
        // parseUrl splits the value on ',' → focus=100,50 arrives as args ["100","50"]
        // (the same way pixijs reads it), NOT one "100,50" string.
        tileX = Number(cmd.args[0]) || 0;
        tileY = Number(cmd.args[1]) || 0;
      } else if (cmd.name === "grid") {
        const on = argFlag(cmd.args[0]);
        this.panel.view.setDebugGrid(on ? Number(cmd.args[0]) || 1 : 0);
      }
    }
    // Centre the anchor + force the first client subscription (login has completed).
    this.bridge.start(tileX, tileY);

    const canvas = this.panel.canvas;
    canvas.addEventListener("pointerdown", this.onPointerDown);
    canvas.addEventListener("pointermove", this.onPointerMove);
    canvas.addEventListener("pointerup", this.onPointerUp);
    canvas.addEventListener("pointercancel", this.onPointerUp);
    canvas.addEventListener("wheel", this.onWheel, { passive: false });
  }

  update(): void {
    this.panel?.tick();
  }

  onExit(): void {
    const canvas = this.panel?.canvas;
    if (canvas) {
      canvas.removeEventListener("pointerdown", this.onPointerDown);
      canvas.removeEventListener("pointermove", this.onPointerMove);
      canvas.removeEventListener("pointerup", this.onPointerUp);
      canvas.removeEventListener("pointercancel", this.onPointerUp);
      canvas.removeEventListener("wheel", this.onWheel);
    }
    this.moverLayer?.dispose();
    this.bridge?.dispose();
    this.panel?.destroy();
  }

  // ── drag-to-pan (through the bridge, so zone subscriptions follow) ────────────────
  private readonly onPointerDown = (e: PointerEvent): void => {
    this.dragId = e.pointerId;
    this.lastClientX = e.clientX;
    this.lastClientY = e.clientY;
    this.panel.canvas.setPointerCapture(e.pointerId);
  };

  private readonly onPointerMove = (e: PointerEvent): void => {
    if (this.dragId !== e.pointerId) return;
    const z = this.panel.view.camera.zoom;
    // Drag right → world slides right under the cursor → anchor moves left. A screen-px drag
    // is 1/zoom world px.
    this.bridge.moveBy((this.lastClientX - e.clientX) / z, (this.lastClientY - e.clientY) / z);
    this.lastClientX = e.clientX;
    this.lastClientY = e.clientY;
  };

  private readonly onPointerUp = (e: PointerEvent): void => {
    if (this.dragId !== e.pointerId) return;
    this.dragId = null;
    try {
      this.panel.canvas.releasePointerCapture(e.pointerId);
    } catch {
      /* capture may already be gone */
    }
  };

  // ── scroll-to-zoom (about the cursor, through the bridge) ─────────────────────────
  // Continuous, per-event zoom (matching pixijs): every wheel tick applies a smooth
  // factor 2^(-deltaY/WHEEL_OCTAVE) — no discrete accumulator (that made small ticks a
  // no-op until they summed past a threshold).
  private readonly onWheel = (e: WheelEvent): void => {
    e.preventDefault();
    const r = this.panel.canvas.getBoundingClientRect();
    const factor = Math.pow(2, -e.deltaY / WHEEL_OCTAVE);
    // zoomAt applies the zoom to the camera + returns the cursor-anchored point; zoomTo pushes
    // it (client subscription + viewport anchor).
    const anchor = this.panel.view.camera.zoomAt(e.clientX - r.left, e.clientY - r.top, factor);
    if (anchor) this.bridge.zoomTo(anchor.x, anchor.y, this.panel.view.camera.zoom);
  };
}
