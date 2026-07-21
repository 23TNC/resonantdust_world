//! WorldScene — W4a: the real world scene, hosting the {@link ViewportPanel} (its own
//! canvas + engine Renderer) with a working camera. Drag to pan, scroll to zoom, the
//! `?grid` debug grid. The world CONTENT (SquareCache G-buffer + albedo display, movers,
//! shadows) plus the WorldBridge/MoverLayer/chat wiring land in W4c/W4d — until then the
//! viewport shows the procedural grid so the camera is verifiable. The URL `x`/`y`/`focus`
//! commands frame the initial anchor; `grid` toggles the overlay.

import { Scene } from "../Scene";
import type { GameContext } from "../../GameContext";
import { ViewportPanel } from "../../game/viewport/ViewportPanel";
import { SQUARE } from "../../game/viewport/squareMath";
import { parseUrl, argFlag } from "../../debug/urlParams";

/** Accumulated wheel `deltaY` that halves or doubles the zoom (one LOD octave). */
const WHEEL_OCTAVE = 240;

export class WorldScene extends Scene {
  private panel!: ViewportPanel;
  private dragId: number | null = null;
  private lastClientX = 0;
  private lastClientY = 0;
  private wheelAccum = 0;

  onEnter(ctx: GameContext): void {
    this.panel = new ViewportPanel(ctx);
    this.panel.open();

    // URL: initial anchor (x/y/focus tile coords) + the debug grid.
    let tileX = 0;
    let tileY = 0;
    for (const cmd of parseUrl().commands) {
      if (cmd.name === "x") tileX = Number(cmd.args[0] ?? 0) || 0;
      else if (cmd.name === "y") tileY = Number(cmd.args[0] ?? 0) || 0;
      else if (cmd.name === "focus") {
        const [fx, fy] = (cmd.args[0] ?? "").split(",");
        tileX = Number(fx) || 0;
        tileY = Number(fy) || 0;
      } else if (cmd.name === "grid") {
        const on = argFlag(cmd.args[0]);
        this.panel.view.setDebugGrid(on ? Number(cmd.args[0]) || 1 : 0);
      }
    }
    this.panel.view.camera.setAnchor(tileX * SQUARE, tileY * SQUARE);

    // TEMP (W4c): a checkerboard of solid tiles to verify the SquareCache bake+display pipeline.
    // Replaced by the WorldBridge (real zone tiles) in W4d.
    for (let ty = 0; ty < 16; ty++) {
      for (let tx = 0; tx < 16; tx++) {
        const color = (tx + ty) & 1 ? 0x3a7d3a : 0x2d5f8f; // green / blue checker
        this.panel.view.debugAddTile(tx, ty, color);
      }
    }

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
    this.panel?.destroy();
  }

  // ── drag-to-pan ────────────────────────────────────────────────────────────────
  private readonly onPointerDown = (e: PointerEvent): void => {
    this.dragId = e.pointerId;
    this.lastClientX = e.clientX;
    this.lastClientY = e.clientY;
    this.panel.canvas.setPointerCapture(e.pointerId);
  };

  private readonly onPointerMove = (e: PointerEvent): void => {
    if (this.dragId !== e.pointerId) return;
    const cam = this.panel.view.camera;
    const dx = (e.clientX - this.lastClientX) / cam.zoom;
    const dy = (e.clientY - this.lastClientY) / cam.zoom;
    this.lastClientX = e.clientX;
    this.lastClientY = e.clientY;
    cam.setAnchor(cam.anchorX - dx, cam.anchorY - dy);
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

  // ── scroll-to-zoom (about the cursor) ────────────────────────────────────────────
  private readonly onWheel = (e: WheelEvent): void => {
    e.preventDefault();
    this.wheelAccum += e.deltaY;
    let factor = 1;
    while (this.wheelAccum <= -WHEEL_OCTAVE) {
      factor *= 2;
      this.wheelAccum += WHEEL_OCTAVE;
    }
    while (this.wheelAccum >= WHEEL_OCTAVE) {
      factor *= 0.5;
      this.wheelAccum -= WHEEL_OCTAVE;
    }
    if (factor === 1) return;
    const r = this.panel.canvas.getBoundingClientRect();
    const anchor = this.panel.view.camera.zoomAt(e.clientX - r.left, e.clientY - r.top, factor);
    if (anchor) this.panel.view.camera.setAnchor(anchor.x, anchor.y);
  };
}
