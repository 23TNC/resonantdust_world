//! The viewport renderer: owns the {@link AlbedoMap} (the toroidal albedo cache) and
//! the display mesh that draws it around the anchor, once per frame.
//!
//! A {@link LayoutNode} so the panel can size it to the body rect. Each frame the
//! owning panel calls {@link tick}: it re-sizes the map to the body, recenters the
//! cache window on the anchor, bakes any dirty squares, then rebuilds the display
//! mesh — one quad per resident world square, panned so the anchor sits at the body
//! centre. The mesh renders as an ordinary child of the Pixi scene graph.
//!
//! ALBEDO-ONLY first slice: the display is a plain textured mesh sampling the albedo
//! composite. The normal / emissive / lighting channels (and the multi-sampler
//! display shader that combines them) are the seam this grows into — `this.mesh`'s
//! shader is the single swap point, and {@link AlbedoMap} gains sibling composites.

import { Buffer, BufferUsage, Geometry, Mesh, Texture } from "pixi.js";
import type { Renderer } from "pixi.js";
import { LayoutNode } from "../layout/LayoutNode";
import type { TextureManager } from "../../textures";
import type { RtChannel } from "../panels/rt/RtPanel";
import { AlbedoMap, type PrimitiveSpec } from "./AlbedoMap";

/** Dirty squares baked per frame. A fresh window dirties its whole grid; the budget
 *  spreads that over a few frames so the first open never hitches. */
const BAKE_BUDGET = 128;

export class Viewport extends LayoutNode {
  private readonly map = new AlbedoMap();

  /** World-px point centred in the viewport. Starts at the origin. */
  private anchorX = 0;
  private anchorY = 0;

  /** Last body size the map was sized to (px). */
  private lastW = 0;
  private lastH = 0;

  // ── display mesh ──────────────────────────────────────────────────────────────
  private mesh: Mesh<Geometry> | null = null;
  private curQuads = -1;
  private pos = new Float32Array(0);
  private uv = new Float32Array(0);
  private posBuf: Buffer | null = null;
  private uvBuf: Buffer | null = null;

  constructor(private readonly textures: TextureManager) {
    super();
  }

  /** Recenter the view on a world-px point. */
  setAnchor(x: number, y: number): void {
    this.anchorX = x;
    this.anchorY = y;
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
      { name: "albedo", texture: this.map.composite },
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
    if (w !== this.lastW || h !== this.lastH) {
      this.map.resize(w, h, renderer);
      this.lastW = w;
      this.lastH = h;
    }
    this.map.recenter(this.anchorX, this.anchorY);
    this.map.bakeDirty(renderer, BAKE_BUDGET);
    if (!this.map.ready) return;

    this.ensureGeometry(this.map.displayQuadCount);
    // Pan so the anchor sits at the body centre — "draw the view around the anchor".
    // Snap the pan to the device-pixel grid (round in device px, back to CSS px) so
    // every quad edge lands on a whole texel: sub-pixel pans otherwise smear the
    // bilinear sample across the grid and shimmer the seams. `roundPixels` on the
    // mesh then snaps the *absolute* vertex positions (this pan + the panel's own
    // fractional offset) so adjacent quads share an exact edge — no MSAA crack.
    const res = renderer.resolution;
    const panX = Math.round((w / 2 - this.anchorX) * res) / res;
    const panY = Math.round((h / 2 - this.anchorY) * res) / res;
    this.map.fillDisplay(panX, panY, this.pos, this.uv);
    this.posBuf!.update();
    this.uvBuf!.update();
    const composite = this.map.composite;
    if (composite && this.mesh) this.mesh.texture = composite;
  }

  /** (Re)allocate the display mesh's buffers when the resident-square count changes.
   *  Indices are a static two-triangle quad list; positions/UVs are filled per frame
   *  by {@link AlbedoMap.fillDisplay}. */
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
    this.map.destroy();
    this.mesh?.geometry.destroy();
    this.posBuf?.destroy();
    this.uvBuf?.destroy();
    super.destroy();
  }
}
