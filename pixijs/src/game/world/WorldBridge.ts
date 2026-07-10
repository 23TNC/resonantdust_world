//! Wiring between the world client and the viewport — the seam that turns the
//! viewport's camera into the client's anchor.
//!
//! The anchor is pushed to the viewport every move (so the camera follows) and to
//! the client whenever it crosses a tile (so the hysteresis ladder subscribes /
//! unsubscribes zones). It also seeds the viewport's static bake config (the noise
//! atlas + the content's material registry), refreshed on a content hot-swap.
//!
//! Painting subscribed zones into sprites is not wired here yet: the merged shard
//! streams only `state` rows, and terrain (cold-zone) rendering re-enters through
//! the zone pipeline as a follow-up (docs/gaps.md). Live entities are drawn by the
//! screen-space `DemoLayer` off the client's `state` stream.

import type { WasmClient, AnchorRadii } from "../../client/WasmClient";
import type { Content } from "../../client/wasm";
import type { Viewport } from "../viewport/Viewport";
import { SQUARE } from "../viewport/squareMath";
import { MaterialRegistry } from "../viewport/material";
import { makeNoiseAtlas } from "../viewport/noiseAtlas";

/** The viewport anchor's name in the client's anchor list. */
const ANCHOR = "viewport:0";

/** How slowly the sticky subscription reach relaxes toward the current zoom's reach —
 *  one tile per this many ms of zoomed-in time. Long enough that a quick zoom in→out
 *  keeps its subs, short enough that a sustained zoom-in lets the outer band fall away. */
const STICKY_RELAX_MS = 200;

/** Whether two radii sets are identical — a zoom that doesn't change the reach skips the
 *  client re-push (and its ladder re-evaluation). */
const sameRadii = (a: AnchorRadii, b: AnchorRadii): boolean =>
  a.active === b.active && a.hot === b.hot && a.warm === b.warm && a.cold === b.cold;

export class WorldBridge {
  /** Anchor centre, in world px. */
  private anchorX = 0;
  private anchorY = 0;
  /** Last anchor tile pushed to the client (NaN before the first push). */
  private anchorTileX = NaN;
  private anchorTileY = NaN;
  /** Current viewport zoom (screen px per world px); scales the subscription reach. */
  private zoom = 1;
  /** Sticky subscription reach (tiles): grows instantly on zoom-out, relaxes slowly on
   *  zoom-in, so a zoom in→out cycle keeps its subs instead of dropping + re-fetching. */
  private stickyActive = 0;
  private stickyRelaxedMs = 0;
  /** Last radii pushed to the client, so an unchanged reach skips the re-push. */
  private lastRadii: AnchorRadii = { active: -1, hot: -1, warm: -1, cold: -1 };

  constructor(
    private readonly client: WasmClient,
    private content: Content,
    private readonly viewport: Viewport,
  ) {
    // The tiling noise atlas the material bake samples — a static, content-independent
    // asset, generated once for the session.
    this.viewport.setNoiseAtlas(makeNoiseAtlas());
    this.refreshMaterials();
  }

  /** Re-read the material registry from the current content bundle — after
   *  construction and on every hot-swap, so the albedo bake sees any newly-authored
   *  materials. */
  private refreshMaterials(): void {
    this.viewport.setMaterialRegistry(
      MaterialRegistry.fromWasm(
        this.content.materialNoiseFields(),
        this.content.materialSampleSpaces(),
        this.content.materialSwings(),
      ),
    );
  }

  /** Begin: place the anchor at the world origin and force the first client
   *  subscription. Call after login. */
  start(): void {
    this.anchorTileX = NaN; // force the first client push
    this.setAnchor(0, 0);
  }

  /** Move the anchor to a world-px point: always recenter the viewport; push to the
   *  client only when the anchor tile OR the reach ({@link radii}) actually changes, so a
   *  pan within a tile and a zoom that doesn't move a tier boundary are both free. */
  setAnchor(x: number, y: number): void {
    this.anchorX = x;
    this.anchorY = y;
    this.viewport.setAnchor(x, y);

    const tileX = Math.round(x / SQUARE);
    const tileY = Math.round(y / SQUARE);
    const r = this.radii();
    if (tileX !== this.anchorTileX || tileY !== this.anchorTileY || !sameRadii(r, this.lastRadii)) {
      this.anchorTileX = tileX;
      this.anchorTileY = tileY;
      this.lastRadii = r;
      this.client.setAnchor(ANCHOR, tileX, tileY, 0, r, 0);
    }
  }

  /** Pan the anchor by a world-px delta (drag handling lives in the scene). */
  moveBy(dx: number, dy: number): void {
    this.setAnchor(this.anchorX + dx, this.anchorY + dy);
  }

  /** Apply a zoom: record it (widens {@link radii} as we zoom out to see more world) and
   *  move to the cursor-anchored point. {@link setAnchor} re-pushes on its own iff the
   *  reach actually changed, so a zoom that stays within a tier boundary is free. */
  zoomTo(x: number, y: number, zoom: number): void {
    this.zoom = zoom;
    this.setAnchor(x, y);
  }

  /** Drop the client anchor. */
  dispose(): void {
    this.client.removeAnchor(ANCHOR);
  }

  /** Swap in a hot-reloaded content bundle so a `.rd` edit (a new material) shows
   *  without a re-fetch. Terrain re-expansion re-enters with the zone pipeline. */
  setContent(content: Content): void {
    this.content = content;
    this.refreshMaterials();
  }

  // ── internals ───────────────────────────────────────────────────────

  /** Per-tier anchor reach in tiles. `active` (opens subs) + `hot` (pan hysteresis)
   *  track the CURRENT zoom: `active` covers the visible area (half the larger screen
   *  dimension in tiles, ÷ zoom — zooming out shows more world) plus a prefetch margin.
   *  `warm`/`cold` (the sticky candidate band) track a STICKY reach that grows instantly
   *  on zoom-out and relaxes slowly on zoom-in, so a quick zoom in→out keeps its subs
   *  (held as warm candidates — the client's warmth + capacity-LRU bound them) instead
   *  of dropping + re-fetching. Called every anchor move; mutates the sticky state. */
  private radii(): AnchorRadii {
    const screenTiles = Math.max(window.innerWidth, window.innerHeight) / (SQUARE * this.zoom);
    const active = Math.ceil(screenTiles / 2) + 2;
    const now = Date.now();
    if (active >= this.stickyActive) {
      this.stickyActive = active; // zoom-out (or first aim): grow to cover instantly
      this.stickyRelaxedMs = now;
    } else {
      // Zoom-in: relax the held reach toward `active` at one tile per STICKY_RELAX_MS,
      // carrying the remainder so the rate is wall-clock, not call-frequency, driven.
      const shrink = Math.floor((now - this.stickyRelaxedMs) / STICKY_RELAX_MS);
      if (shrink > 0) {
        this.stickyActive = Math.max(active, this.stickyActive - shrink);
        this.stickyRelaxedMs += shrink * STICKY_RELAX_MS;
      }
    }
    return { active, hot: active + 2, warm: this.stickyActive + 4, cold: this.stickyActive + 6 };
  }
}
