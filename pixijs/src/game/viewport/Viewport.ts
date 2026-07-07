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
//! DEFERRED LIGHTING display: the cache bakes albedo + normal + depth channels, and the
//! mesh draws with {@link LightingShader}, which samples the normal + albedo composites
//! and lights per-fragment (ambient + directional sun today; point lights + depth shadows
//! extend the shader in place). The depth composite is bound once the shadow pass lands.

import { Buffer, BufferUsage, Geometry, Mesh, Texture } from "pixi.js";
import type { Renderer } from "pixi.js";
import { LayoutNode } from "../layout/LayoutNode";
import type { TextureResolver } from "../../textures";
import { ZOOM_MAX, ZOOM_MIN } from "../../textures/lod";
import type { RtChannel } from "../panels/rt/RtPanel";
import { SquareCache, type PrimitiveSpec } from "./SquareCache";
import { NOISE_FIELDS, PACKED_UNIFORM_LEN, type MaterialRegistry } from "./material";
import { SQUARE } from "./squareMath";
import { makeLightingShader, type LightingShader } from "./lightingShader";
import { LightRig } from "../lighting/LightRig";
import { ShadowPass, type ShadowCaster } from "./shadowPass";

/** Dirty squares baked per frame. A fresh window dirties its whole grid; the budget
 *  spreads that over a few frames so the first open never hitches. */
const BAKE_BUDGET = 128;

/** The wedge's ground angle (0 = flat / depth vertical, π/2 = standing / depth N–S). */
const GROUND_ANGLE = (65 * Math.PI) / 180;

/** The caster's ORIGIN as a fraction down the sprite box (0 = top, 1 = bottom): the bottom-most
 *  fully-present pixel down the SURFACE map's centre column (its B channel is the coverage) — the
 *  trunk-ground contact the shadow's foot plants on. Pixi discards the CPU bitmap after GPU
 *  upload, so we read the pixels back through the renderer's `extract`. Returns null on failure
 *  (caller falls back to the box bottom). Computed once per stem and cached. */
function originFrac(renderer: Renderer, texture: Texture): number | null {
  try {
    const out = renderer.extract.pixels(texture) as { pixels: Uint8ClampedArray | Uint8Array; width: number; height: number };
    const { pixels, width, height } = out;
    if (!width || !height || !pixels) return null;
    const cx = Math.min(width - 1, Math.floor(width / 2));
    for (let y = height - 1; y >= 0; y--) {
      if (pixels[(y * width + cx) * 4 + 2] >= 250) return (y + 1) / height; // B = coverage
    }
    return 1; // fully transparent centre column → no crop
  } catch {
    return null;
  }
}

/** A world-y's DEPTH on the 0..1 ring: one step per {@link DEPTH_PX} world px, wrapped over the
 *  full 0..255 range. The composite/shadow ALPHA marks thing-vs-ground (so the value can start at
 *  0), and the lighting compare handles the ring wrap — so nothing needs a ≥128 floor. Baked into
 *  the depth composite (receiver) and the shadow RT's R (caster). Finer than per-tile so things
 *  a few px apart in y still order (and occlude) correctly. */
const DEPTH_PX = 5;
function depthUnit(worldY: number): number {
  return (((Math.floor(worldY / DEPTH_PX) % 256) + 256) % 256) / 255;
}

/** Zero material-channel params (no jitter) — shared by the albedo path when a prim has no
 *  varying materials (the reconstruction is then just the residual). Read-only. */
const ZERO_CH = new Float32Array(PACKED_UNIFORM_LEN);

export class Viewport extends LayoutNode {
  /** The world cache — bakes albedo + normal + depth channels from one prim index. The
   *  display still samples only albedo (Phase A); the lighting shader consumes the rest. */
  private readonly map: SquareCache;

  /** World-px point centred in the viewport. Starts at the origin. */
  private anchorX = 0;
  private anchorY = 0;

  /** Screen px per world px. 1 = tiles at native 64px; >1 zoomed in, <1 out. */
  private zoomFactor = 1;

  // ── display mesh (one quad per resident world square) ───────────────────────────
  private mesh: Mesh<Geometry> | null = null;
  /** The deferred lighting material the display mesh draws with — samples the cache's
   *  normal + albedo composites and lights per-fragment (ambient + sun; point lights +
   *  shadows in later phases). Owned here, bound with the live composites each frame. */
  private readonly lightingShader: LightingShader = makeLightingShader();
  /** The light rig — sun + ambient + dynamic point lights — packed into the shader each
   *  frame. Callers reach it via {@link lights} to register world lights / drive the cursor. */
  private readonly rig = new LightRig();
  /** The wedge shadow pass — rasterizes billboard shadows into a screen-space coverage RT
   *  (viewable as the `shadow` channel in `/showRT`). Not yet sampled by the lighting pass. */
  private readonly shadowPass = new ShadowPass();
  /** Per-stem shadow ORIGIN fraction (bottom-most opaque pixel down the centre line), computed
   *  once from the texture pixels and reused every frame. */
  private readonly originCache = new Map<string, number>();
  private curQuads = -1;
  private pos = new Float32Array(0);
  private uv = new Float32Array(0);
  private posBuf: Buffer | null = null;
  private uvBuf: Buffer | null = null;

  /** The texture resolver — the viewport tells it the target LOD as the zoom moves. */
  private readonly resolver: TextureResolver;
  /** Drop the resolver's load subscription on destroy. */
  private readonly unsubLoad: () => void;

  /** The material registry (params by id, from the `<material>` DSL) — the albedo bake
   *  reads it to add hue/chroma variation to a prim's packed channels. `null` until the
   *  content bundle is loaded ({@link setMaterialRegistry}); until then every prim bakes
   *  flat, exactly as before. */
  private materialRegistry: MaterialRegistry | null = null;

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
          // The `albedo` map is the residual base (RGB). Its visual alpha lives in `surface.B`,
          // so the reconstruction needs BOTH loaded before it can composite — until then the
          // prim stays a flat geo box (a solid rectangle, as the geo tier always was), never a
          // silhouette-less residual box.
          const alb = resolver.resolve(prim.textureName, "albedo", prim.cell);
          const surf = resolver.resolve(prim.textureName, "surface", prim.cell);
          if (alb.geo || surf.geo) return { texture: alb.texture, tint: prim.geoColor ?? prim.tint };
          // Single real-tier path: reconstruct residual + Σ layers·jitter, alpha from surface.B.
          // Only sample the `layers` map when a bound material actually varies; otherwise the
          // residual IS the full albedo (layers = null → residual only).
          const reg = this.materialRegistry;
          let layers: Texture | null = null;
          let chA: Float32Array = ZERO_CH;
          let chB: Float32Array = ZERO_CH;
          if (reg && reg.channelsVary(prim.packed)) {
            const lyr = resolver.resolve(prim.textureName, "layers", prim.cell);
            if (!lyr.geo) {
              layers = lyr.texture;
              ({ chA, chB } = reg.packChannels(prim.packed));
            }
          }
          return { texture: alb.texture, tint: prim.tint, material: { residual: alb.texture, layers, surface: surf.texture, chA, chB } };
        },
      },
      // The normal channel: a real normal map bakes UNTINTED (0xffffff) so the albedo tint
      // never skews the vector; where a stem has no normal (most art), fall back to flat-up
      // +Z — RGB (128,128,255) = 0x8080ff tinted onto the white fill (the old game's
      // makeFlatNormal, via the existing tint machinery, no dedicated texture).
      {
        key: "normal",
        resolve: (prim) => {
          // Standing things: premultiply the normal (real LOD, or flat-up) by surface.B — the
          // soft coverage — so the flat background never clobbers, and the over-blend onto the
          // ground normal computes the α-lerp. Needs surface loaded (the coverage source);
          // until then, fall through to the flat sprite.
          if (prim.zIndex >= 1 && prim.textureName) {
            const surf = resolver.resolve(prim.textureName, "surface", prim.cell);
            if (!surf.geo) {
              const n = resolver.resolve(prim.textureName, "normal", prim.cell);
              return { texture: surf.texture, tint: 0xffffff, normal: { rgb: n.geo ? null : n.texture, alpha: surf.texture } };
            }
          }
          // Ground / tiles / geo-tier things: full flat-up (or a real tile normal) as before.
          if (!prim.textureName) return { texture: resolver.white, tint: 0x8080ff };
          const { texture, geo } = resolver.resolve(prim.textureName, "normal", prim.cell);
          return geo ? { texture: resolver.white, tint: 0x8080ff } : { texture, tint: 0xffffff };
        },
      },
      // The surface channel: R = height (reserved; unused by the current lighting), G = ambient
      // occlusion, B = coverage/transparency. A real map bakes through the presence-premultiply
      // shader (composite A = presence); ground / geo-tier things degrade to a flat OPAQUE,
      // PRESENT, un-occluded fill (0x00ffff = height 0, ao 1, coverage 1) as a plain sprite.
      {
        key: "surface",
        resolve: (prim) => {
          if (!prim.textureName) return { texture: resolver.white, tint: 0x00ffff };
          const { texture, geo } = resolver.resolve(prim.textureName, "surface", prim.cell);
          return geo ? { texture: resolver.white, tint: 0x00ffff } : { texture, tint: 0xffffff, surface: true };
        },
      },
      // The depth channel: B = the prim's tile-depth (row%256, /255), for STANDING things only —
      // ground/tiles + geo-tier bake nothing. Baked once, dirty-tracked, world-space; the
      // lighting pass reads B at vUV and omits a shadow where it's ≥ the caster depth. The
      // silhouette/presence comes from the SURFACE map's B (via the depth bake's smoothstep).
      {
        key: "depth",
        resolve: (prim) => {
          if (prim.zIndex >= 1 && prim.textureName) {
            const surf = resolver.resolve(prim.textureName, "surface", prim.cell);
            if (!surf.geo) return { texture: surf.texture, tint: 0xffffff, depth: depthUnit(prim.y + prim.height) };
          }
          return { texture: resolver.white, tint: 0xffffff, depth: -1 }; // ground / geo → write nothing
        },
      },
    ]);
    // When a tier lands, re-dirty every prim'd square so the bake upgrades.
    this.unsubLoad = resolver.onLoad(() => this.map.invalidateAll());
  }

  /** Swap in the material registry (from the content bundle; re-set on hot-swap) and
   *  re-bake so the albedo picks up any newly-authored material variation. */
  setMaterialRegistry(registry: MaterialRegistry): void {
    this.materialRegistry = registry;
    this.map.invalidateAll();
  }

  /** Bind the tiling noise atlas the material bake samples. `rows` is the field count
   *  (atlas rows). A `null` texture disables the material path. Re-bakes so it takes. */
  setNoiseAtlas(texture: Texture | null, rows: number = NOISE_FIELDS.length): void {
    // uvTile = 1 noise tile across a sprite; worldTile = 2 world tiles (128px) per noise
    // tile, so ground mottle spans a couple of cells before repeating.
    this.map.setNoise(texture, rows, 1, SQUARE * 2);
    this.map.invalidateAll();
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

  /** The light rig — register world-space point lights (mutate them to move a torch), set
   *  the sun/ambient, or drive the cursor light. Lights are in WORLD px. */
  get lights(): LightRig {
    return this.rig;
  }

  /** Map a body-local screen point (px, origin at the viewport's top-left) to a WORLD-px
   *  point — the same inverse transform {@link zoomAt} uses. Feed a pointermove through
   *  this into {@link LightRig.setCursorWorld} to attach a hover light. */
  screenToWorld(sx: number, sy: number): { x: number; y: number } {
    const z = this.zoomFactor;
    return { x: this.anchorX + (sx - this.width / 2) / z, y: this.anchorY + (sy - this.height / 2) / z };
  }

  /** Named render-texture channels for the `/showRT` dev preview. Stable order so
   *  the preview tiles don't reshuffle as channels light up: `albedo` is the live
   *  fixed-slot composite (the map itself); `normal`/`emissive` are dormant until
   *  their composites come online (the multi-channel bake), reporting `null`. */
  renderTextures(): RtChannel[] {
    return [
      { name: "albedo", texture: this.map.displayComposite("albedo") },
      { name: "normal", texture: this.map.displayComposite("normal") },
      { name: "surface", texture: this.map.displayComposite("surface") },
      { name: "depth", texture: this.map.displayComposite("depth") },
      { name: "shadow", texture: this.shadowPass.texture },
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
    // Bind the live composites to the lighting shader (both share the same swap state, so
    // they stay consistent through a LOD swap). The mesh's main texture IS the normal
    // composite (textureBit samples it); albedo is a second sampler the shader reads.
    const albedo = this.map.displayComposite("albedo");
    const normal = this.map.displayComposite("normal");
    const surface = this.map.displayComposite("surface");
    const depth = this.map.displayComposite("depth");
    if (albedo && normal && surface && depth && this.mesh) {
      this.lightingShader.albedo = albedo;
      this.lightingShader.normal = normal;
      this.lightingShader.surface = surface;
      this.lightingShader.depth = depth;
      // The mesh's aPosition is the PANNED world coord (worldX + panX); world-space lights
      // add this pan to line up with the fragment's vWorld.
      this.lightingShader.setPan(panX, panY);
    }
    // Upload the sun + ambient + dynamic point lights for this frame.
    this.rig.pack(this.lightingShader);

    // Shadow pass: rasterize wedge shadows for the first shadow-casting light into the coverage
    // RT (body px, so it aligns 1:1 with the display), then bind it to the lighting shader so
    // the display pass darkens the direct light where shadowed.
    const shLight = this.rig.shadowCasters()[0];
    const casters = shLight ? this.buildCasters(renderer) : [];
    if (shLight && casters.length) {
      this.shadowPass.render(renderer, w, h, 1, casters, shLight, GROUND_ANGLE, (wx) => (wx + panX) * z, (wy) => (wy + panY) * z);
    } else {
      this.shadowPass.clear(renderer, w, h, 1);
    }
    const shadowTex = this.shadowPass.texture;
    if (shadowTex && this.mesh) {
      this.lightingShader.shadow = shadowTex;
      // vWorld = worldX + panX; bodyPx = vWorld·zoom; shadow uv = bodyPx / bodySize. The last
      // pair is the Y flip (0,0 = none) — dialed in against the RT's sample orientation.
      this.lightingShader.setShadowUv(z / w, z / h, 0, 0);
    }
  }

  /** The shadow-pass caster list: each standing prim's billboard box + its resolved SURFACE
   *  texture (its B channel is the silhouette/coverage the shadow rasterizes) + a placeholder
   *  depth. Skips prims still on the geo tier (no surface) so only true silhouettes cast. */
  private buildCasters(renderer: Renderer): ShadowCaster[] {
    const out: ShadowCaster[] = [];
    for (const p of this.map.standingPrims()) {
      if (!p.textureName) continue;
      const { texture, geo } = this.resolver.resolve(p.textureName, "surface", p.cell);
      if (geo) continue;
      // Origin (trunk base) fraction — read back from the GPU once per stem, then cached
      // (fall back to the box bottom if the readback fails, so the caster still shadows). A
      // linked cell keys by (stem, cell) since each cell has its own silhouette.
      const okey = p.cell != null ? `${p.textureName}#${p.cell}` : p.textureName;
      let footFrac = this.originCache.get(okey);
      if (footFrac === undefined) {
        footFrac = originFrac(renderer, texture) ?? 1;
        this.originCache.set(okey, footFrac);
      }
      out.push({
        x: p.x,
        y: p.y,
        width: p.width,
        height: p.height,
        texture,
        flipX: !!p.flipX,
        depth: Math.max(4, p.width * 0.15), // placeholder until the DSL / depth-map wiring
        footFrac,
        // Same depth encoding as the `depth` composite (base y) so a thing's shadow matches its
        // own receiver depth exactly → it never self-shadows.
        tileDepth: depthUnit(p.y + p.height),
      });
    }
    return out;
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
      this.mesh = new Mesh<Geometry>({ geometry: geo, shader: this.lightingShader });
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
    this.shadowPass.destroy();
    this.mesh?.geometry.destroy();
    this.lightingShader.destroy();
    this.posBuf?.destroy();
    this.uvBuf?.destroy();
    super.destroy();
  }
}
