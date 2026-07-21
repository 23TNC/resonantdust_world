//! Camera — the viewport's world↔screen transform. Anchor (world-px point centred in
//! the viewport) + zoom (screen px per world px), with the same screen↔world maths the
//! pixijs `Viewport` carried, lifted out so the render layer and the input handlers share
//! one source of truth. Pure — no GL, no DOM.

import { ZOOM_MAX, ZOOM_MIN } from "../../textures/lod";

export class Camera {
  /** World-px point centred in the viewport. Starts at the origin. */
  anchorX = 0;
  anchorY = 0;
  /** Viewport size in CSS px. */
  width = 0;
  height = 0;
  /** Screen px per world px. 1 = tiles at native 64px; >1 zoomed in, <1 out. */
  private zoomFactor = 1;

  get zoom(): number {
    return this.zoomFactor;
  }

  setBounds(width: number, height: number): void {
    this.width = width;
    this.height = height;
  }

  setAnchor(x: number, y: number): void {
    this.anchorX = x;
    this.anchorY = y;
  }

  /** Body-local screen px → world px. */
  screenToWorld(sx: number, sy: number): { x: number; y: number } {
    const z = this.zoomFactor;
    return { x: this.anchorX + (sx - this.width / 2) / z, y: this.anchorY + (sy - this.height / 2) / z };
  }

  /** World px → body-local screen px (inverse of {@link screenToWorld}). */
  worldToScreen(wx: number, wy: number): { x: number; y: number } {
    const z = this.zoomFactor;
    return { x: (wx - this.anchorX) * z + this.width / 2, y: (wy - this.anchorY) * z + this.height / 2 };
  }

  /** Zoom by `factor` about the body-local point `(sx,sy)` — the world point under the
   *  cursor stays fixed. Returns the new anchor the caller must apply (so zone
   *  subscriptions follow), or null if the zoom clamped to a no-op. */
  zoomAt(sx: number, sy: number, factor: number): { x: number; y: number } | null {
    const old = this.zoomFactor;
    const z = Math.min(Math.max(old * factor, ZOOM_MIN), ZOOM_MAX);
    if (z === old) return null;
    const cx = this.width / 2;
    const cy = this.height / 2;
    const wx = this.anchorX + (sx - cx) / old;
    const wy = this.anchorY + (sy - cy) / old;
    this.zoomFactor = z;
    return { x: wx - (sx - cx) / z, y: wy - (sy - cy) / z };
  }

  /** Set an absolute zoom, holding the viewport centre fixed. Returns the (unchanged)
   *  anchor to push through the bridge, or null if it clamped to a no-op. */
  setZoom(z: number): { x: number; y: number } | null {
    return this.zoomAt(this.width / 2, this.height / 2, z / this.zoomFactor);
  }
}
