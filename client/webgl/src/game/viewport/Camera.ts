//! Camera — the viewport's world↔screen transform. Anchor (world-px point centred in
//! the viewport) + zoom (screen px per world px), with the same screen↔world maths the
//! pixijs `Viewport` carried, lifted out so the render layer and the input handlers share
//! one source of truth. Pure — no GL, no DOM.

import { ZOOM_MAX, ZOOM_MIN } from "../../textures/lod";
import { coverScale } from "./squareMath";

export class Camera {
  /** World-px point centred in the viewport. Starts at the origin. */
  anchorX = 0;
  anchorY = 0;
  /** Viewport size in CSS px. */
  width = 0;
  height = 0;
  /** LOGICAL zoom — 1 = a tile at its native `SQUARE` px on the REFERENCE target, and the dial the lod
   *  ladder reads. Deliberately NOT screen px per world px: that is {@link renderScale}. Keeping the
   *  logical zoom monitor-independent is what puts every player on the same lod at the same zoom. */
  private zoomFactor = 1;
  /** The COVER fit for the current viewport (`max(W/2560, H/1536)`) — see {@link renderScale}. */
  private cover = 1;

  get zoom(): number {
    return this.zoomFactor;
  }

  /** Screen px per world px = logical zoom × the cover fit. This is what every world↔screen transform
   *  uses. The cover term is what makes the visible world IDENTICAL on every monitor: at logical zoom 1
   *  a player sees exactly `VISIBLE_X × VISIBLE_Y` slots whatever their resolution — a larger display
   *  magnifies rather than revealing more world (work `2026-07-26-textile-slot` F1/F4). */
  get renderScale(): number {
    return this.zoomFactor * this.cover;
  }

  setBounds(width: number, height: number): void {
    this.width = width;
    this.height = height;
    this.cover = width > 0 && height > 0 ? coverScale(width, height) : 1;
  }

  setAnchor(x: number, y: number): void {
    this.anchorX = x;
    this.anchorY = y;
  }

  /** Body-local screen px → world px. */
  screenToWorld(sx: number, sy: number): { x: number; y: number } {
    const z = this.renderScale;
    return { x: this.anchorX + (sx - this.width / 2) / z, y: this.anchorY + (sy - this.height / 2) / z };
  }

  /** World px → body-local screen px (inverse of {@link screenToWorld}). */
  worldToScreen(wx: number, wy: number): { x: number; y: number } {
    const z = this.renderScale;
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
    // Hold the world point under the cursor fixed — in RENDER scale, since that is the transform the
    // cursor is actually looking through. `cover` is constant across the zoom, so it does not cancel.
    const wx = this.anchorX + (sx - cx) / (old * this.cover);
    const wy = this.anchorY + (sy - cy) / (old * this.cover);
    this.zoomFactor = z;
    return { x: wx - (sx - cx) / (z * this.cover), y: wy - (sy - cy) / (z * this.cover) };
  }

  /** Set an absolute zoom, holding the viewport centre fixed. Returns the (unchanged)
   *  anchor to push through the bridge, or null if it clamped to a no-op. */
  setZoom(z: number): { x: number; y: number } | null {
    return this.zoomAt(this.width / 2, this.height / 2, z / this.zoomFactor);
  }
}
