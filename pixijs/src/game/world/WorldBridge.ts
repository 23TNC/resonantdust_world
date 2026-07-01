//! Wiring between the world client and the viewport — the seam that turns
//! subscribed zones into painted tiles, and the viewport's camera into the
//! client's anchor.
//!
//! On every cold-zone delivery (`onZoneTiles`) it runs the DSL (`Content
//! .zoneTilePrims`, which executes each tile's `:visual @on_create`) to expand
//! the packed slots into `[tileX, tileY, tint]` triples, then adds one 64×64
//! white-texture sprite per non-empty cell, tinted by its colour. On a zone
//! close it drops that zone's sprites. The anchor is pushed to the viewport every
//! move (so the camera follows) and to the client whenever it crosses a tile (so
//! the hysteresis ladder subscribes / unsubscribes zones).

import type { Texture } from "pixi.js";
import type { WasmClient, AnchorRadii } from "../../client/WasmClient";
import type { Content } from "../../client/wasm";
import type { Viewport } from "../viewport/Viewport";
import { SQUARE } from "../viewport/squareMath";

/** The viewport anchor's name in the client's anchor list. */
const ANCHOR = "viewport:0";

export class WorldBridge {
  /** Sprite ids per subscribed zone, so a close drops exactly that zone. */
  private readonly zonePrims = new Map<number, number[]>();
  private readonly unsubs: Array<() => void> = [];

  /** Anchor centre, in world px. */
  private anchorX = 0;
  private anchorY = 0;
  /** Last anchor tile pushed to the client (NaN before the first push). */
  private anchorTileX = NaN;
  private anchorTileY = NaN;

  constructor(
    private readonly client: WasmClient,
    private readonly content: Content,
    private readonly viewport: Viewport,
    private readonly white: Texture,
  ) {
    this.unsubs.push(client.onZoneTiles((zoneId, tiles) => this.onZoneTiles(zoneId, tiles)));
    this.unsubs.push(client.onZoneClosed((zoneId) => this.onZoneClosed(zoneId)));
  }

  /** Begin: place the anchor at the world origin and force the first client
   *  subscription. Call after login. */
  start(): void {
    this.anchorTileX = NaN; // force the first client push
    this.setAnchor(0, 0);
  }

  /** Move the anchor to a world-px point: always recenter the viewport; push to
   *  the client only when the anchor tile changes (subscriptions are per tile). */
  setAnchor(x: number, y: number): void {
    this.anchorX = x;
    this.anchorY = y;
    this.viewport.setAnchor(x, y);

    const tileX = Math.round(x / SQUARE);
    const tileY = Math.round(y / SQUARE);
    if (tileX !== this.anchorTileX || tileY !== this.anchorTileY) {
      this.anchorTileX = tileX;
      this.anchorTileY = tileY;
      this.client.setAnchor(ANCHOR, tileX, tileY, 0, this.radii(), 0);
    }
  }

  /** Pan the anchor by a world-px delta (drag handling lives in the scene). */
  moveBy(dx: number, dy: number): void {
    this.setAnchor(this.anchorX + dx, this.anchorY + dy);
  }

  /** Drop everything: listeners, every zone's sprites, and the client anchor. */
  dispose(): void {
    for (const unsub of this.unsubs) unsub();
    this.unsubs.length = 0;
    for (const ids of this.zonePrims.values()) {
      for (const id of ids) this.viewport.removePrim(id);
    }
    this.zonePrims.clear();
    this.client.removeAnchor(ANCHOR);
  }

  // ── internals ───────────────────────────────────────────────────────

  /** Per-tier anchor reach in tiles: the `active` ring covers the visible area
   *  (half the larger screen dimension, in tiles) plus a prefetch margin; the
   *  hysteresis tiers widen outward from there. */
  private radii(): AnchorRadii {
    const screenTiles = Math.max(window.innerWidth, window.innerHeight) / SQUARE;
    const active = Math.ceil(screenTiles / 2) + 2;
    return { active, hot: active + 2, warm: active + 4, cold: active + 6 };
  }

  /** A zone's cold baseline arrived: re-seed its sprites from the DSL expansion. */
  private onZoneTiles(zoneId: number, tiles: Uint16Array): void {
    // A later cold row rewrites the zone — clear the old sprites first.
    this.onZoneClosed(zoneId);

    const flat = this.content.zoneTilePrims(zoneId, tiles); // [tileX, tileY, tint, …]
    const ids: number[] = [];
    for (let i = 0; i + 2 < flat.length; i += 3) {
      const tileX = flat[i];
      const tileY = flat[i + 1];
      const tint = flat[i + 2];
      ids.push(
        this.viewport.addPrim({
          texture: this.white,
          x: tileX * SQUARE,
          y: tileY * SQUARE,
          width: SQUARE,
          height: SQUARE,
          tint,
        }),
      );
    }
    if (ids.length) this.zonePrims.set(zoneId, ids);
  }

  /** A zone's subscription closed (or it's being re-seeded): drop its sprites. */
  private onZoneClosed(zoneId: number): void {
    const ids = this.zonePrims.get(zoneId);
    if (!ids) return;
    for (const id of ids) this.viewport.removePrim(id);
    this.zonePrims.delete(zoneId);
  }
}
