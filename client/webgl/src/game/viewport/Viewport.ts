//! The viewport renderer (webgl) — owns its own `<canvas>` + engine {@link Renderer}, the
//! {@link Camera}, and the cold {@link SquareCache} G-buffer. Each frame: resize + recenter the
//! toroidal cache on the anchor, bake dirty squares (merged MRT bake), then draw one quad per
//! resident world square with the {@link AlbedoBlitShader}, panned + zoomed via `uProjection`.
//! A procedural world-grid draws on top when `?grid` is on.
//!
//! W4c/W4d slice: UNLIT albedo display, geo tier only (every prim bakes as a solid `geoColor`
//! box — white fill × tint), so the world renders without the texture atlas. The warm (mover)
//! tier, real texture resolution, shadows, and the debug overlays are later slices.

import { Renderer, Program, Geometry, Texture } from "../../gl";
import { Camera } from "./Camera";
import { SquareCache, type PrimitiveSpec, type ChannelSpec, type Primitive } from "./SquareCache";
import { AlbedoBlitShader } from "./albedoBlitShader";
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
  private readonly empty: Texture;
  private materialRegistry: MaterialRegistry | null = null;

  private readonly map: SquareCache;
  /** The WARM cache — same channels keyed `*-warm`, baked from the MOVER prims (pawns). Driven
   *  with the identical window/slot geometry as {@link map}, so its composites are slot-aligned
   *  and the display blit composites warm OVER cold by warm coverage. */
  private readonly warm: SquareCache;
  private readonly blitShader: AlbedoBlitShader;
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
    this.empty = new Texture(gl, { width: 1, height: 1, format: "rgba8unorm", data: new Uint8Array([0, 0, 0, 0]) });
    this.blitShader = new AlbedoBlitShader(gl);
    this.map = new SquareCache(this.renderer, this.empty, this.channels("cold"));
    this.warm = new SquareCache(this.renderer, this.empty, this.channels("warm"));

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
      { key: `albedo-${suffix}`, resolve: (prim) => ({ texture: white, tint: prim.geoColor ?? prim.tint, material: { residual: white, layers: null, surface: white, chA: ZERO_CH, chB: ZERO_CH } }) },
      { key: `normal-${suffix}`, resolve: () => ({ texture: white, tint: 0x8080ff }) },
      { key: `surface-${suffix}`, resolve: () => ({ texture: white, tint: 0x00ffff }) },
      { key: `zdepth-world-${suffix}`, resolve: () => ({ texture: white, tint: 0xffffff, depth: -1 }) },
    ];
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
      }
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
    this.blitShader.destroy();
    this.map.destroy();
    this.warm.destroy();
    this.white.destroy();
    this.empty.destroy();
  }
}
