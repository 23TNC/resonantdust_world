import { Container, Graphics, type Ticker } from "pixi.js";
import type { StateObject } from "../../client/WasmClient";

/** Mirrors `resonantdust_codec::packed::OBJ_TYPE_DEMO` — the object_id type tag the
 *  demo mover stamps. Only these are drawn. */
const OBJ_TYPE_DEMO = 1;

const GRID = 16; // a zone is 16×16 cells; `location = cy<<4 | cx`
const CELL = 28; // px per cell in the overlay
const PAD = 12;
const SIZE = GRID * CELL;
/** Distinct colours per demo serial (wraps if there are more than 6). */
const COLORS = [0xff5a5a, 0x5ada6a, 0x5a9cff, 0xffce4a, 0xc86bff, 0x4adfd0];

interface Circle {
  g: Graphics;
  x: number;
  y: number;
  tx: number;
  ty: number;
}

/**
 * A fixed screen-space overlay that draws each demo object (from the object-shard
 * `state` stream) as a circle on a 16×16 grid and **tweens** it toward each new
 * `location`. Deliberately camera-independent — two browser tabs render the identical
 * overlay, so a divergence between them is a sync bug, plainly visible. (Placing these
 * on the world map proper is a later refinement; this is the sync probe.)
 */
export class DemoLayer {
  readonly container = new Container();
  private readonly circles = new Map<number, Circle>();

  constructor() {
    const bg = new Graphics();
    bg.rect(0, 0, SIZE + PAD * 2, SIZE + PAD * 2).fill({ color: 0x05070a, alpha: 0.55 });
    bg.rect(PAD, PAD, SIZE, SIZE).stroke({ color: 0x55617a, alpha: 0.7, width: 1 });
    this.container.addChild(bg);
    // Top-left corner, below any title bar.
    this.container.position.set(20, 60);
  }

  private cellCenter(location: number): { x: number; y: number } {
    const cx = location & (GRID - 1);
    const cy = (location >> 4) & (GRID - 1);
    return { x: PAD + cx * CELL + CELL / 2, y: PAD + cy * CELL + CELL / 2 };
  }

  /** Apply a `state` change: create/move/remove the circle for this demo serial. */
  upsert(obj: StateObject): void {
    if (obj.objType !== OBJ_TYPE_DEMO) return;
    const existing = this.circles.get(obj.serial);
    if (obj.removed) {
      if (existing) {
        existing.g.destroy();
        this.circles.delete(obj.serial);
      }
      return;
    }
    const { x, y } = this.cellCenter(obj.location);
    if (!existing) {
      const g = new Graphics();
      g.circle(0, 0, CELL * 0.34).fill({
        color: COLORS[obj.serial % COLORS.length],
        alpha: 0.95,
      });
      g.position.set(x, y);
      this.container.addChild(g);
      this.circles.set(obj.serial, { g, x, y, tx: x, ty: y });
      return;
    }
    existing.tx = x;
    existing.ty = y;
  }

  /** Per-frame tween toward each circle's target (call from the app ticker). */
  update(ticker: Ticker): void {
    const k = Math.min(1, ticker.deltaMS / 180); // ~180ms to converge
    for (const c of this.circles.values()) {
      c.x += (c.tx - c.x) * k;
      c.y += (c.ty - c.y) * k;
      c.g.position.set(c.x, c.y);
    }
  }

  destroy(): void {
    this.container.destroy({ children: true });
    this.circles.clear();
  }
}
