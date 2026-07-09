//! Sync-experiment overlay — draws one circle per `experiment_objects` row and
//! tweens it toward each new position the server sends.
//!
//! This is deliberately PARALLEL to the real sync path (`WorldBridge` + the
//! synced clock + `valid_at` projection; see docs/sync.md). It touches none of
//! that: it owns a plain `Container` of `Graphics` circles, positions them each
//! frame via the viewport camera ({@link Viewport.worldToScreen}), and tweens on
//! its OWN local clock — no render delay, no interpolation-to-shared-instant. The
//! point is to exercise module → server → wasm client → pixijs end-to-end with the
//! simplest possible renderer.

import { Container, Graphics } from "pixi.js";
import type { WasmClient, ExperimentObject } from "../../client/WasmClient";
import type { Viewport } from "../viewport/Viewport";
import { SQUARE } from "../viewport/squareMath";

/** Tween duration from an object's current position to a freshly-received one.
 *  A touch under the npc's ~2 s update cadence so a circle settles before its
 *  next hop. A local wall-clock duration — NOT the synced render delay. */
const TWEEN_MS = 1500;

/** Circle radius in WORLD px (scaled by zoom at draw time), ~⅓ of a tile. */
const RADIUS_WORLD = SQUARE * 0.34;

/** Distinct fill colours cycled by `objectId`, so the five objects are telling
 *  apart at a glance. */
const COLORS = [0xff5a5a, 0x5ad1ff, 0x8bff5a, 0xffd05a, 0xc98bff];

/** Per-object render state. `from`/`to`/`start` describe the active tween in
 *  fractional TILE coordinates; `cur` is the last interpolated tile position,
 *  reused as the `from` when a new target arrives mid-tween. */
interface CircleState {
  g: Graphics;
  fromX: number;
  fromY: number;
  toX: number;
  toY: number;
  curX: number;
  curY: number;
  start: number;
}

export class ExperimentLayer {
  /** The overlay root — the owning scene adds this to its stage container. */
  readonly container = new Container();

  private readonly circles = new Map<number, CircleState>();
  private readonly unsub: () => void;

  constructor(
    private readonly client: WasmClient,
    private readonly view: Viewport,
  ) {
    this.container.label = "experiment-overlay";
    // The viewport lazily adds its terrain mesh as a sibling of this overlay
    // (after we attach), so raise the overlay above it — the parent enables
    // `sortableChildren` (see WorldScene) so this zIndex takes effect.
    this.container.zIndex = 1000;
    this.unsub = client.onExperiment((obj) => this.onExperiment(obj));
  }

  /** A row arrived (insert or update): create the circle if new, else retarget
   *  its tween from wherever it currently sits toward `(x, y)`. */
  private onExperiment(obj: ExperimentObject): void {
    const now = performance.now();
    const existing = this.circles.get(obj.objectId);
    if (!existing) {
      const g = new Graphics().circle(0, 0, RADIUS_WORLD).fill({
        color: COLORS[obj.objectId % COLORS.length],
        alpha: 0.9,
      });
      // A thin dark ring so a circle reads against bright terrain.
      g.circle(0, 0, RADIUS_WORLD).stroke({ color: 0x101418, width: 2, alpha: 0.8 });
      this.container.addChild(g);
      // Seeded rows arrive with the object at rest — no tween, just place it.
      this.circles.set(obj.objectId, {
        g,
        fromX: obj.x,
        fromY: obj.y,
        toX: obj.x,
        toY: obj.y,
        curX: obj.x,
        curY: obj.y,
        start: now,
      });
      return;
    }
    // Retarget: tween from the current interpolated point (not the old target),
    // so a hop that lands mid-tween doesn't snap backwards.
    existing.fromX = existing.curX;
    existing.fromY = existing.curY;
    existing.toX = obj.x;
    existing.toY = obj.y;
    existing.start = now;
  }

  /** Per-frame: advance each tween and place its circle under the current camera.
   *  Called by the scene's update loop, before the viewport bakes. */
  tick(): void {
    const now = performance.now();
    const zoom = this.view.zoom;
    for (const st of this.circles.values()) {
      const f = Math.min(1, Math.max(0, (now - st.start) / TWEEN_MS));
      st.curX = st.fromX + (st.toX - st.fromX) * f;
      st.curY = st.fromY + (st.toY - st.fromY) * f;
      // Tile → world px (centre of the tile) → screen px under pan/zoom.
      const wx = (st.curX + 0.5) * SQUARE;
      const wy = (st.curY + 0.5) * SQUARE;
      const s = this.view.worldToScreen(wx, wy);
      st.g.position.set(s.x, s.y);
      // The circle is drawn in world px; scaling by zoom keeps it locked to the
      // world (grows/shrinks with the terrain instead of floating at fixed size).
      st.g.scale.set(zoom);
    }
  }

  /** Drop the subscription and destroy the circles. */
  destroy(): void {
    this.unsub();
    for (const st of this.circles.values()) st.g.destroy();
    this.circles.clear();
    this.container.destroy({ children: true });
  }
}
