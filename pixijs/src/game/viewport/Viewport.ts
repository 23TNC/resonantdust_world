//! The viewport renderer: owns the {@link SquareCache} (the toroidal world cache) and
//! the display mesh that draws it around the anchor, once per frame.
//!
//! A {@link LayoutNode} so the panel can size it to the body rect. Each frame the
//! owning panel calls {@link tick}: it re-sizes the cache to the visible world extent
//! (body px ÷ zoom), recenters the cache window on the anchor (a toroidal strip
//! re-bake, no full copy), bakes any dirty squares, then rebuilds the display mesh —
//! one quad per resident world square, panned around the anchor and scaled by the
//! zoom. The mesh renders as an ordinary child of the Pixi scene graph.
//!
//! ALBEDO-ONLY display for now: the mesh samples the cache's `albedo` channel. The
//! cache already bakes an arbitrary CHANNEL list; adding normal / emissive is a channel
//! push, and the multi-sampler lighting shader is the remaining display-side seam —
//! `this.mesh`'s shader is the single swap point.

import { Buffer, BufferUsage, Geometry, Mesh, Texture } from "pixi.js";
import type { Renderer } from "pixi.js";
import { LayoutNode } from "../layout/LayoutNode";
import type { TextureResolver } from "../../textures";
import { ZOOM_MAX, ZOOM_MIN } from "../../textures/lod";
import type { RtChannel } from "../panels/rt/RtPanel";
import { SquareCache, type PrimitiveSpec } from "./SquareCache";
import { SQUARE } from "./squareMath";

/** Dirty squares baked per frame. A fresh window dirties its whole grid; the budget
 *  spreads that over a few frames so the first open never hitches. */
const BAKE_BUDGET = 128;

export class Viewport extends LayoutNode {
  /** The world cache — one albedo channel today; normal / emissive slot in beside it. */
  private readonly map: SquareCache;

  /** World-px point centred in the viewport. Starts at the origin. */
  private anchorX = 0;
  private anchorY = 0;

  /** Screen px per world px. 1 = tiles at native 64px; >1 zoomed in, <1 out. */
  private zoomFactor = 1;

  // ── display mesh (one quad per resident world square) ───────────────────────────
  private mesh: Mesh<Geometry> | null = null;
  private curQuads = -1;
  private pos = new Float32Array(0);
  private uv = new Float32Array(0);
  private posBuf: Buffer | null = null;
  private uvBuf: Buffer | null = null;

  /** The texture resolver — the viewport tells it the target LOD as the zoom moves. */
  private readonly resolver: TextureResolver;
  /** Drop the resolver's load subscription on destroy. */
  private readonly unsubLoad: () => void;

  constructor(resolver: TextureResolver) {
    super();
    this.resolver = resolver;
    // The albedo channel: bake each prim through the three-tier resolver — a named prim
    // gets the best texture on hand (master → preview → geo); an unnamed tint-rect keeps
    // its own texture. Colour by tier — geo shows the prim's silhouette colour
    // (`geoColor`, falling back to `tint`); a loaded tier shows `tint`. (A future normal
    // channel is another entry here with its own resolve + flat-up geo fallback.)
    this.map = new SquareCache([
      {
        key: "albedo",
        resolve: (prim) => {
          if (!prim.textureName) return { texture: prim.texture, tint: prim.tint };
          const { texture, geo } = resolver.resolve(prim.textureName);
          return { texture, tint: geo ? prim.geoColor ?? prim.tint : prim.tint };
        },
      },
    ]);
    // When a tier lands, re-dirty every prim'd square so the bake upgrades.
    this.unsubLoad = resolver.onLoad(() => this.map.invalidateAll());
  }

  /** Recenter the view on a world-px point. */
  setAnchor(x: number, y: number): void {
    this.anchorX = x;
    this.anchorY = y;
  }

  /** Current zoom (screen px per world px). */
  get zoom(): number {
    return this.zoomFactor;
  }

  /** Zoom by `factor` about the body-local point `(sx,sy)` — the world point under
   *  the cursor stays fixed. Returns the new anchor the caller must apply (through
   *  the world bridge, so zone subscriptions follow), or null if the zoom clamped to
   *  a no-op. The zoom itself is applied here. */
  zoomAt(sx: number, sy: number, factor: number): { x: number; y: number } | null {
    const old = this.zoomFactor;
    const z = Math.min(Math.max(old * factor, ZOOM_MIN), ZOOM_MAX);
    if (z === old) return null;
    const cx = this.width / 2;
    const cy = this.height / 2;
    // World point under the cursor before the zoom …
    const wx = this.anchorX + (sx - cx) / old;
    const wy = this.anchorY + (sy - cy) / old;
    this.zoomFactor = z;
    // … must map back under the cursor after it.
    return { x: wx - (sx - cx) / z, y: wy - (sy - cy) / z };
  }

  /** Add a renderable primitive to the albedo map, returning its id (the world
   *  scene feeds real tiles here). */
  addPrim(spec: PrimitiveSpec): number {
    return this.map.addPrim(spec);
  }

  /** Remove a primitive previously added with {@link addPrim}. */
  removePrim(id: number): void {
    this.map.removePrim(id);
  }

  /** Named render-texture channels for the `/showRT` dev preview. Stable order so
   *  the preview tiles don't reshuffle as channels light up: `albedo` is the live
   *  fixed-slot composite (the map itself); `normal`/`emissive` are dormant until
   *  their composites come online (the multi-channel bake), reporting `null`. */
  renderTextures(): RtChannel[] {
    return [
      { name: "albedo", texture: this.map.displayComposite("albedo") },
      { name: "normal", texture: null },
      { name: "emissive", texture: null },
    ];
  }

  /** Display aspect ratio (width / height) — the `/showRT` preview sizes its tiles
   *  to match. 0 before the first layout. */
  aspect(): number {
    return this.height > 0 ? this.width / this.height : 0;
  }

  /** Per-frame: resize → recenter → bake → rebuild the display mesh. The panel calls
   *  this every frame (after it has synced this node's bounds to the body rect). */
  tick(renderer: Renderer): void {
    const w = Math.floor(this.width);
    const h = Math.floor(this.height);
    if (w <= 0 || h <= 0) return;
    // The composite is a FIXED-size buffer sized to the viewport; the LOD only changes the
    // SLOT size (a world square is drawn into a derived `slotPx`-CSS-px rect), so crossing
    // a LOD boundary changes the slot size + count, not the RT size. The display quad —
    // world px — is scaled up by the zoom.
    const z = this.zoomFactor;
    // Aim texture loads at the on-screen tile size (a tile is SQUARE world px).
    this.resolver.setTargetLod(SQUARE * z);
    this.map.resize(w, h, renderer, z, this.anchorX, this.anchorY);
    this.map.recenter(this.anchorX, this.anchorY);
    this.map.bakeDirty(renderer, BAKE_BUDGET);
    if (!this.map.ready) return;

    this.ensureGeometry(this.map.displayQuadCount);
    // Pan so the anchor sits at the body centre (in WORLD px — the mesh scale maps
    // world → screen). Snap to whole CSS px (the composite is CSS-res); `roundPixels`
    // then snaps the absolute (scaled) vertices to device px.
    const worldW = w / z;
    const worldH = h / z;
    const panX = Math.round(worldW / 2 - this.anchorX);
    const panY = Math.round(worldH / 2 - this.anchorY);
    this.map.fillDisplay(panX, panY, this.pos, this.uv);
    this.posBuf!.update();
    this.uvBuf!.update();
    if (this.mesh) this.mesh.scale.set(z);
    const composite = this.map.displayComposite("albedo");
    if (composite && this.mesh) this.mesh.texture = composite;
  }

  /** (Re)allocate the display mesh's buffers when the resident-square count changes.
   *  Indices are a static two-triangle quad list; positions/UVs are filled per frame
   *  by {@link SquareCache.fillDisplay}. */
  private ensureGeometry(quads: number): void {
    if (quads === this.curQuads && this.mesh) return;
    this.pos = new Float32Array(quads * 8);
    this.uv = new Float32Array(quads * 8);
    const idx = new Uint32Array(quads * 6);
    for (let q = 0; q < quads; q++) {
      const v = q * 4;
      const o = q * 6;
      idx[o] = v; idx[o + 1] = v + 1; idx[o + 2] = v + 2;
      idx[o + 3] = v; idx[o + 4] = v + 2; idx[o + 5] = v + 3;
    }
    this.posBuf = new Buffer({ data: this.pos, usage: BufferUsage.VERTEX | BufferUsage.COPY_DST });
    this.uvBuf = new Buffer({ data: this.uv, usage: BufferUsage.VERTEX | BufferUsage.COPY_DST });
    const geo = new Geometry({
      attributes: {
        aPosition: { buffer: this.posBuf, format: "float32x2" },
        aUV: { buffer: this.uvBuf, format: "float32x2" },
      },
      indexBuffer: new Buffer({ data: idx, usage: BufferUsage.INDEX | BufferUsage.COPY_DST }),
    });
    if (!this.mesh) {
      this.mesh = new Mesh<Geometry>({ geometry: geo, texture: Texture.EMPTY });
      // Snap final vertex positions to whole device pixels in the shader, so the
      // grid's shared edges coincide exactly (no sub-pixel crack between quads).
      this.mesh.roundPixels = true;
      this.container.addChild(this.mesh);
    } else {
      const old = this.mesh.geometry;
      this.mesh.geometry = geo;
      old.destroy();
    }
    this.curQuads = quads;
  }

  override destroy(): void {
    this.unsubLoad();
    this.map.destroy();
    this.mesh?.geometry.destroy();
    this.posBuf?.destroy();
    this.uvBuf?.destroy();
    super.destroy();
  }
}
