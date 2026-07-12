//! The mover layer — the tick pipeline's mobile entities (pawns) drawn as a
//! screen-space overlay ABOVE the baked world.
//!
//! Phase 1 of pawns: each pawn (a wolf) is a plain, always-bright `Graphics` circle, so
//! we can prove spawn + placement before any sprite/facing work. Unlike the static
//! tiles/things (which bake into the viewport's lit prim cache), movers live on the
//! viewport's unlit {@link Viewport.overlay}: a marker is positioned from its WORLD centre
//! via {@link Viewport.worldToScreen} — on each `state` update (so it follows the pawn) and
//! every frame in {@link tick} (so it follows the camera as it pans/zooms), and it is never
//! dimmed by the world lighting. A `StateObject` upserts a marker (keyed by `objectId`);
//! `removed` or its zone closing drops it. Later phases swap the circle for the wolf sprite
//! and add facing.

import { Graphics } from "pixi.js";
import type { WasmClient, StateObject } from "../../client/WasmClient";
import type { Content } from "../../client/wasm";
import type { Viewport } from "../viewport/Viewport";
import { SQUARE } from "../viewport/squareMath";

/** The entity class drawn here. `objType` is the entity-key tag from
 *  `resonantdust_codec::refs`; PAWN (3) is the wolf. */
const ENTITY_TYPE_PAWN = 3;

/** Marker radius in WORLD px at zoom 1 (scaled by the zoom each frame). ~a third of a tile. */
const MARKER_WORLD_RADIUS = SQUARE * 0.28;

/** A warm marker colour, distinct from the green flora, so pawns read clearly against the
 *  dark forest. (Debug-only — replaced by the wolf sprite in phase 3.) */
const MARKER_COLOR = 0xff5533;

/** One live pawn marker: its display object + last-known world centre (for the per-frame
 *  reposition) + zone (so a zone close drops it). */
interface Mover {
  gfx: Graphics;
  worldX: number;
  worldY: number;
  zoneId: number;
}

export class MoverLayer {
  private readonly movers = new Map<number, Mover>();
  private readonly unsubs: Array<() => void> = [];

  constructor(
    private readonly client: WasmClient,
    private content: Content,
    private readonly viewport: Viewport,
  ) {
    this.unsubs.push(client.onStateObject((obj) => this.onStateObject(obj)));
    // A zone leaving the subscription sends no per-entity delete, so drop its movers.
    this.unsubs.push(client.onZoneClosed((zoneId) => this.onZoneClosed(zoneId)));
  }

  /** A content hot-swap: `moverPrim` (position + tint) comes from the bundle, so re-point it. */
  setContent(content: Content): void {
    this.content = content;
  }

  /** Reposition every marker for the current camera. Call once per frame, AFTER the viewport
   *  ticks (so its anchor/zoom are current for this frame). */
  tick(): void {
    const z = this.viewport.zoom;
    for (const m of this.movers.values()) {
      const p = this.viewport.worldToScreen(m.worldX, m.worldY);
      m.gfx.position.set(p.x, p.y);
      m.gfx.scale.set(z);
    }
  }

  dispose(): void {
    for (const u of this.unsubs) u();
    this.unsubs.length = 0;
    for (const m of this.movers.values()) m.gfx.destroy();
    this.movers.clear();
  }

  // ── internals ───────────────────────────────────────────────────────

  private onStateObject(obj: StateObject): void {
    if (obj.objType !== ENTITY_TYPE_PAWN) return; // only wolves for now
    const key = obj.objectId;
    if (obj.removed) {
      this.remove(key);
      return;
    }
    // World-px CENTRE of the pawn's cell — `moverPrim` returns the world tile (+ tint/geo).
    const prim = this.content.moverPrim(obj.zoneId, obj.location, obj.objKind);
    const worldX = prim[0] * SQUARE + SQUARE / 2;
    const worldY = prim[1] * SQUARE + SQUARE / 2;

    let m = this.movers.get(key);
    if (!m) {
      const gfx = new Graphics().circle(0, 0, MARKER_WORLD_RADIUS).fill(MARKER_COLOR);
      this.viewport.overlay.addChild(gfx);
      m = { gfx, worldX, worldY, zoneId: obj.zoneId };
      this.movers.set(key, m);
    } else {
      m.worldX = worldX;
      m.worldY = worldY;
      m.zoneId = obj.zoneId;
    }
    // Place it now (don't wait for the next frame) so it appears at the right spot at once.
    const p = this.viewport.worldToScreen(worldX, worldY);
    m.gfx.position.set(p.x, p.y);
    m.gfx.scale.set(this.viewport.zoom);
  }

  private remove(key: number): void {
    const m = this.movers.get(key);
    if (m) {
      m.gfx.destroy();
      this.movers.delete(key);
    }
  }

  private onZoneClosed(zoneId: number): void {
    for (const [key, m] of this.movers) {
      if (m.zoneId === zoneId) {
        m.gfx.destroy();
        this.movers.delete(key);
      }
    }
  }
}
