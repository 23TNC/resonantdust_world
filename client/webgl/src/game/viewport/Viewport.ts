//! The viewport renderer (webgl) — owns its own `<canvas>` + engine {@link Renderer}, the
//! {@link Camera}, and the cold {@link SquareCache} G-buffer. Each frame: resize + recenter the
//! toroidal cache on the anchor, bake dirty squares (merged MRT bake), then draw one quad per
//! resident world square with the {@link AlbedoBlitShader}, panned + zoomed via `uProjection`.
//! A procedural world-grid draws on top when `?grid` is on.
//!
//! W4c/W4d slice: UNLIT albedo display, geo tier only (every prim bakes as a solid `geoColor`
//! box — white fill × tint), so the world renders without the texture atlas. The warm (mover)
//! tier, real texture resolution, shadows, and the debug overlays are later slices.

import { Renderer, Program, Geometry, Texture, TexFrame } from "../../gl";
import { Camera } from "./Camera";
import { SquareCache, type PrimitiveSpec, type ChannelSpec, type Primitive } from "./SquareCache";
import type { TextureResolver } from "../../textures";
import { AlbedoBlitShader } from "./albedoBlitShader";
import { OverlayShader, overlayModeFor } from "./overlayShader";
import { ShadowCaster } from "./shadowCaster";
import { PACKED_CHANNELS } from "./mrtBakeShader";
import type { MaterialRegistry } from "./material";
import { SQUARE, ZONE_DIM, REGION_DIM } from "./squareMath";
import { ZOOM_MAX, ZOOM_MIN } from "../../textures/lod";

const BAKE_BUDGET = 128;
/** Minimum cold squares baked per frame regardless of warm load — so a warm flood can't
 *  starve the static world. */
const COLD_BAKE_FLOOR = 64;
/** Zero material channels (no jitter) — the geo path bakes flat. */
const ZERO_CH = new Float32Array(PACKED_CHANNELS * 4);

const GRID_VERT = /* glsl */ `#version 300 es
in vec2 aPosition;
out vec2 vClip;
void main() { vClip = aPosition; gl_Position = vec4(aPosition, 0.0, 1.0); }
`;
const GRID_FRAG = /* glsl */ `#version 300 es
precision highp float;
in vec2 vClip;
uniform vec2  uViewport;
uniform vec2  uAnchor;
uniform float uZoom;
uniform vec3  uPitch;
uniform float uLevel;
out vec4 fragColor;
float line(vec2 w, float pitch, float halfPx) {
  vec2 m = abs(mod(w, pitch));
  m = min(m, pitch - m);
  vec2 sp = m * uZoom;
  float d = min(sp.x, sp.y);
  return 1.0 - smoothstep(halfPx - 1.0, halfPx + 1.0, d);
}
void main() {
  vec2 fragPx = vec2((vClip.x * 0.5 + 0.5) * uViewport.x, (0.5 - vClip.y * 0.5) * uViewport.y);
  vec2 world = uAnchor + (fragPx - uViewport * 0.5) / uZoom;
  vec3 col = vec3(0.0);
  float a = 0.0;
  if (uLevel <= 1.5) { float t = line(world, uPitch.x, 1.0) * 0.30; col = mix(col, vec3(1.0, 0.25, 0.25), t); a = max(a, t); }
  if (uLevel <= 2.5) { float z = line(world, uPitch.y, 1.5) * 0.45; col = mix(col, vec3(1.0, 0.30, 1.0), z); a = max(a, z); }
  { float r = line(world, uPitch.z, 2.0) * 0.65; col = mix(col, vec3(0.30, 0.55, 1.0), r); a = max(a, r); }
  fragColor = vec4(col, a);
}
`;

export class Viewport {
  readonly camera = new Camera();
  private readonly renderer: Renderer;
  /** The 1×1 white fill — the geo-tier residual/surface + the WorldBridge's prim texture. */
  readonly white: Texture;
  /** The white fill as an identity frame — the geo tier's residual/surface (whole-texture UV). */
  private whiteFrame!: TexFrame;
  private readonly empty: Texture;
  private materialRegistry: MaterialRegistry | null = null;
  /** The texture resolver (master→preview→geo). Null until the world scene attaches it + our GL
   *  context ({@link setResolver}); the geo tier renders until then. */
  private resolver: TextureResolver | null = null;
  private resolverUnsub: (() => void) | null = null;

  private readonly map: SquareCache;
  /** The WARM cache — same channels keyed `*-warm`, baked from the MOVER prims (pawns). Driven
   *  with the identical window/slot geometry as {@link map}, so its composites are slot-aligned
   *  and the display blit composites warm OVER cold by warm coverage. */
  private readonly warm: SquareCache;
  private readonly blitShader: AlbedoBlitShader;
  /** The `/overlayRT` debug material — draws one G-buffer composite over the display. */
  private readonly overlayShader: OverlayShader;
  /** The composite the overlay is currently showing (e.g. `normal-cold`), or null (off). */
  private overlayChannelName: string | null = null;
  /** The first lights + billboard shadows (cast off the cold cache's standing prims). */
  private readonly shadows: ShadowCaster;
  private displayGeo: Geometry | null = null;
  private pos = new Float32Array(0);
  private uv = new Float32Array(0);
  private curQuads = -1;

  private readonly grid: Program;
  private readonly gridQuad: Geometry;
  private gridLevel = 0;

  constructor() {
    this.renderer = new Renderer();
    this.renderer.canvas.style.cssText = "display:block;width:100%;height:100%;";
    const gl = this.renderer.gl;
    this.white = new Texture(gl, { width: 1, height: 1, format: "rgba8unorm", data: new Uint8Array([255, 255, 255, 255]) });
    this.whiteFrame = TexFrame.whole(this.white);
    this.empty = new Texture(gl, { width: 1, height: 1, format: "rgba8unorm", data: new Uint8Array([0, 0, 0, 0]) });
    this.blitShader = new AlbedoBlitShader(gl);
    this.overlayShader = new OverlayShader(gl);
    this.map = new SquareCache(this.renderer, this.empty, this.channels("cold"));
    this.warm = new SquareCache(this.renderer, this.empty, this.channels("warm"));
    this.shadows = new ShadowCaster(this.renderer);

    this.grid = new Program(gl, GRID_VERT, GRID_FRAG, "viewport-grid");
    this.gridQuad = new Geometry(gl, this.grid, {
      aPosition: { data: new Float32Array([-1, -1, 3, -1, -1, 3]), size: 2 },
    });
  }

  /** The four G-buffer channels' resolve hooks. Geo tier only (W4c): every prim resolves to a
   *  solid material — white fill × `geoColor` tint, coverage 1 (never discards). Real texture
   *  resolution (master→preview→geo) returns in a later slice. */
  private channels(suffix: string): ChannelSpec[] {
    const white = this.white;
    return [
      {
        key: `albedo-${suffix}`,
        resolve: (prim) => {
          // Universal-material (mrt-bakes B2): every prim resolves to a material, one bake path. The geo
          // case is a SOLID material — residual × geoColor, a WHITE surface (coverage 1 → never discards →
          // fills the box). A real material passes tint = white (its tint lives in chA/chB). `residual`
          // defaults to the white fill; a partially-loaded stem (albedo up, surface not yet) uses the real
          // albedo so it doesn't flash white (matching pixijs).
          const wf = this.whiteFrame;
          const solid = (residual: TexFrame) => ({ texture: white, tint: prim.geoColor ?? prim.tint, material: { residual, layers: null, surface: wf, chA: ZERO_CH, chB: ZERO_CH } });
          const r = this.resolver;
          if (!prim.textureName || !r) return solid(wf);
          // The albedo map is the residual base; the visual alpha lives in surface.B — need BOTH real.
          const alb = r.resolve(prim.textureName, "albedo", prim.cell);
          const surf = r.resolve(prim.textureName, "surface", prim.cell);
          if (alb.geo || surf.geo || !alb.frame || !surf.frame) return solid(alb.frame ?? wf);
          // Real tier: reconstruct residual + Σ layers·jitter (from the material registry), alpha from
          // surface.B. A stem with no `layers` map keeps just the residual.
          let layers: TexFrame | null = null;
          let chA = ZERO_CH;
          let chB = ZERO_CH;
          const reg = this.materialRegistry;
          if (reg) {
            const lyr = r.resolve(prim.textureName, "layers", prim.cell);
            if (!lyr.geo && lyr.frame) {
              layers = lyr.frame;
              ({ chA, chB } = reg.packChannels(prim.packed));
            }
          }
          return { texture: white, tint: 0xffffff, material: { residual: alb.frame, layers, surface: surf.frame, chA, chB } };
        },
      },
      {
        key: `normal-${suffix}`,
        resolve: (prim) => {
          const r = this.resolver;
          if (!prim.textureName || !r) return { texture: white, tint: 0x8080ff };
          const n = r.resolve(prim.textureName, "normal", prim.cell);
          return { texture: white, tint: 0x8080ff, normal: { rgb: n.geo ? null : n.frame } };
        },
      },
      { key: `surface-${suffix}`, resolve: () => ({ texture: white, tint: 0x00ffff }) },
      { key: `zdepth-world-${suffix}`, resolve: () => ({ texture: white, tint: 0xffffff, depth: -1 }) },
    ];
  }

  /** Attach the texture resolver (master→preview→geo) + our GL context. Called by the world scene once
   *  the viewport exists (F6). A LOD landing re-bakes both caches so prims pick up the upgrade. */
  setResolver(resolver: TextureResolver): void {
    this.resolver = resolver;
    resolver.attachRenderer(this.renderer);
    this.resolverUnsub?.();
    this.resolverUnsub = resolver.onLoad(() => {
      this.map.invalidateAll();
      this.warm.invalidateAll();
    });
    this.map.invalidateAll();
    this.warm.invalidateAll();
  }

  get canvas(): HTMLCanvasElement {
    return this.renderer.canvas;
  }

  setBounds(width: number, height: number): void {
    this.camera.setBounds(width, height);
  }
  /** Recenter on a world-px point (the WorldBridge drives this as the camera pans/zooms). */
  setAnchor(x: number, y: number): void {
    this.camera.setAnchor(x, y);
  }
  /** The material registry (from the content bundle). Stored for when real textures land; the
   *  geo tier bakes flat regardless. Re-bakes so a hot-swap takes. */
  setMaterialRegistry(registry: MaterialRegistry): void {
    this.materialRegistry = registry;
    this.map.invalidateAll();
  }
  /** Bind the tiling noise atlas the material bake samples (null = flat). Re-bakes. */
  setNoiseAtlas(texture: Texture | null, rows = 1): void {
    this.map.setNoise(texture, rows, 1, SQUARE * 2);
    this.map.invalidateAll();
  }
  setDebugGrid(level: number): void {
    this.gridLevel = level;
  }
  get debugGrid(): number {
    return this.gridLevel;
  }

  // ── G-buffer debug composites (`/overlayRT`, `/showRT`) ───────────────────────────
  /** The WORLD-space G-buffer composites — cold (static world) then warm (movers). All are
   *  slot-aligned to the display, so all are sampleable through the display geometry. `/showRT`
   *  previews them; `/overlayRT` draws one over the lit world. A composite may be null before its
   *  cache has laid out (the caller skips those). */
  renderTextures(): Array<{ name: string; texture: Texture | null }> {
    return [
      { name: "albedo-cold", texture: this.map.displayComposite("albedo-cold") },
      { name: "normal-cold", texture: this.map.displayComposite("normal-cold") },
      { name: "surface-cold", texture: this.map.displayComposite("surface-cold") },
      { name: "zdepth-world-cold", texture: this.map.displayComposite("zdepth-world-cold") },
      { name: "albedo-warm", texture: this.warm.displayComposite("albedo-warm") },
      { name: "normal-warm", texture: this.warm.displayComposite("normal-warm") },
      { name: "surface-warm", texture: this.warm.displayComposite("surface-warm") },
      { name: "zdepth-world-warm", texture: this.warm.displayComposite("zdepth-world-warm") },
    ];
  }

  /** The channels `/overlayRT` can show — the names from {@link renderTextures}. */
  overlayChannelNames(): string[] {
    return this.renderTextures().map((c) => c.name);
  }

  /** The channel the overlay is currently showing, or null (off). */
  get overlayChannel(): string | null {
    return this.overlayChannelName;
  }

  /** Toggle/select the `/overlayRT` composite: null or the current name turns it off; a known name
   *  switches to it; an unknown name is a no-op (returns null). Returns the resulting channel. */
  setOverlay(name: string | null): string | null {
    if (name === null || name === this.overlayChannelName) {
      this.overlayChannelName = null;
    } else if (this.overlayChannelNames().includes(name)) {
      this.overlayChannelName = name;
    } else {
      return null; // unknown channel — leave the current overlay untouched
    }
    return this.overlayChannelName;
  }

  /** The composite for a channel name (`*-warm` → the warm cache, else cold). */
  private compositeFor(name: string): Texture | null {
    return name.endsWith("-warm") ? this.warm.displayComposite(name) : this.map.displayComposite(name);
  }

  // ── lights + shadows (`/coldlights`) ──────────────────────────────────────────────
  /** Re-seed the shadow lights in a ring around a tile (defaults to the zone the design uses, 100,50). */
  seedLights(tileX: number, tileY: number): void {
    this.shadows.seed(tileX, tileY);
  }
  /** Toggle the shadow pass; returns whether it's now on. */
  toggleShadows(): boolean {
    return this.shadows.toggle();
  }
  /** Current zoom (screen px per world px). */
  get zoom(): number {
    return this.camera.zoom;
  }
  /** Set an absolute zoom, holding the viewport centre; returns the anchor to push
   *  through the bridge (so zone subscriptions follow), or null if it clamped to a no-op. */
  setZoom(z: number): { x: number; y: number } | null {
    return this.camera.setZoom(z);
  }
  screenToWorld(sx: number, sy: number): { x: number; y: number } {
    return this.camera.screenToWorld(sx, sy);
  }
  worldToScreen(wx: number, wy: number): { x: number; y: number } {
    return this.camera.worldToScreen(wx, wy);
  }

  // ── prim index (the world bridge / test feeds these) ─────────────────────────────
  addPrim(spec: PrimitiveSpec): number {
    return this.map.addPrim(spec);
  }
  removePrim(id: number): void {
    this.map.removePrim(id);
  }
  movePrim(id: number, x: number, y: number): void {
    const prim = this.map.getPrim(id);
    if (!prim || (prim.x === x && prim.y === y)) return;
    prim.x = x;
    prim.y = y;
    this.map.refreshPrim(id);
  }
  getPrim(id: number): Primitive | null {
    return this.map.getPrim(id);
  }

  // ── warm (mover) prim index — the MoverLayer feeds pawns here ─────────────────────
  warmAddPrim(spec: PrimitiveSpec): number {
    return this.warm.addPrim(spec);
  }
  warmGetPrim(id: number): Primitive | null {
    return this.warm.getPrim(id);
  }
  warmRefreshPrim(id: number): void {
    this.warm.refreshPrim(id);
  }
  warmRemovePrim(id: number): void {
    this.warm.removePrim(id);
  }

  /** Per-frame: resize buffer → recenter cache → bake dirty → rebuild + draw the display, then
   *  the debug grid. */
  tick(): void {
    this.renderer.resize();
    const w = Math.floor(this.renderer.canvas.clientWidth);
    const h = Math.floor(this.renderer.canvas.clientHeight);
    if (w <= 0 || h <= 0) return;
    this.camera.setBounds(w, h);
    const z = this.camera.zoom;
    const ax = this.camera.anchorX;
    const ay = this.camera.anchorY;
    // Aim texture loads at the on-screen tile size (a tile is SQUARE world px) so the resolver
    // upgrades toward the zoom's target LOD.
    this.resolver?.setTargetLod(SQUARE * z);

    // Both caches share the window/slot geometry (identical inputs), so their composites stay
    // slot-aligned for the warm-over-cold display blit.
    this.warm.resize(w, h, z, ax, ay);
    this.map.resize(w, h, z, ax, ay);
    this.warm.recenter(ax, ay);
    this.map.recenter(ax, ay);
    // Warm has priority (movers — few — update first); cold gets the remaining budget, floored.
    this.warm.bakeDirty(BAKE_BUDGET);
    this.map.bakeDirty(Math.max(BAKE_BUDGET - this.warm.lastBaked, COLD_BAKE_FLOOR));

    this.renderer.clearScreen(0.05, 0.06, 0.08, 1.0);

    if (this.map.ready) {
      this.ensureGeometry(this.map.displayQuadCount);
      this.map.fillDisplay(0, 0, this.pos, this.uv); // raw world px; uProjection applies pan+zoom
      this.displayGeo!.update("aPosition", this.pos);
      this.displayGeo!.update("aUV", this.uv);
      const albedo = this.map.displayComposite("albedo-cold");
      const surface = this.map.displayComposite("surface-cold");
      if (albedo && surface) {
        this.blitShader.albedo = albedo;
        this.blitShader.surface = surface;
        // Warm tier (movers): slot-aligned with cold, composited OVER cold per-fragment by warm
        // surface.B. Empty (coverage 0 → pure cold) until the warm cache has baked a mover.
        const albedoW = this.warm.displayComposite("albedo-warm");
        const surfaceW = this.warm.displayComposite("surface-warm");
        if (albedoW && surfaceW) {
          this.blitShader.albedoWarm = albedoW;
          this.blitShader.surfaceWarm = surfaceW;
        }
        // world px → clip: x = (wx-ax)*2z/w, y = -(wy-ay)*2z/h  (screen y-down → clip y-up)
        const proj = new Float32Array([
          (2 * z) / w, 0, 0,
          0, -(2 * z) / h, 0,
          -ax * (2 * z) / w, ay * (2 * z) / h, 1,
        ]);
        this.renderer.draw({
          program: this.blitShader.program,
          geometry: this.displayGeo!,
          blend: "normal",
          textures: this.blitShader.textures(this.empty),
          uniforms: (p) => p.uMat3("uProjection", proj),
        });

        // Debug overlay (`/overlayRT`): draw one G-buffer composite over the lit world, in exact
        // register (same display geometry + `uProjection`). The fragment drops the channel's "empty"
        // value so the world reads through elsewhere.
        const ov = this.overlayChannelName;
        if (ov) {
          const comp = this.compositeFor(ov);
          if (comp) {
            this.overlayShader.composite = comp;
            this.overlayShader.mode = overlayModeFor(ov);
            this.renderer.draw({
              program: this.overlayShader.program,
              geometry: this.displayGeo!,
              blend: "normal",
              textures: this.overlayShader.textures(this.empty),
              uniforms: (p) => {
                p.uMat3("uProjection", proj);
                p.uFloat("uMode", this.overlayShader.mode);
              },
            });
          }
        }
      }
    }

    // The first lights + billboard shadows, cast off the cold cache's standing prims, over the world.
    if (this.map.ready) {
      this.shadows.tick(this.camera, this.map.standingPrims(), this.resolver);
    }

    if (this.gridLevel > 0) {
      this.renderer.draw({
        program: this.grid,
        geometry: this.gridQuad,
        blend: "normal",
        uniforms: (p) => {
          p.uVec2("uViewport", w, h);
          p.uVec2("uAnchor", ax, ay);
          p.uFloat("uZoom", z);
          p.uVec3("uPitch", SQUARE, SQUARE * ZONE_DIM, SQUARE * ZONE_DIM * REGION_DIM);
          p.uFloat("uLevel", this.gridLevel);
        },
      });
    }
  }

  private ensureGeometry(quads: number): void {
    if (quads === this.curQuads && this.displayGeo) return;
    this.pos = new Float32Array(quads * 8);
    this.uv = new Float32Array(quads * 8);
    const idx = new Uint32Array(quads * 6);
    for (let q = 0; q < quads; q++) {
      const v = q * 4;
      const o = q * 6;
      idx[o] = v; idx[o + 1] = v + 1; idx[o + 2] = v + 2;
      idx[o + 3] = v; idx[o + 4] = v + 2; idx[o + 5] = v + 3;
    }
    this.displayGeo?.destroy();
    this.displayGeo = new Geometry(this.renderer.gl, this.blitShader.program, {
      aPosition: { data: this.pos, size: 2 },
      aUV: { data: this.uv, size: 2 },
    }, idx);
    this.curQuads = quads;
  }

  // clamp helper kept for parity with the pixijs camera bounds (unused directly here)
  static clampZoom(z: number): number {
    return Math.min(Math.max(z, ZOOM_MIN), ZOOM_MAX);
  }

  destroy(): void {
    this.displayGeo?.destroy();
    this.gridQuad.destroy();
    this.grid.destroy();
    this.resolverUnsub?.();
    this.blitShader.destroy();
    this.overlayShader.destroy();
    this.shadows.destroy();
    this.map.destroy();
    this.warm.destroy();
    this.white.destroy();
    this.empty.destroy();
  }
}
