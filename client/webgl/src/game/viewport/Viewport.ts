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
import { ShadowGather, AMBIENT_LEVEL } from "./shadowGather";
import { PACKED_CHANNELS } from "./mrtBakeShader";
import type { MaterialRegistry } from "./material";
import { SQUARE, ZONE_DIM, REGION_DIM, TEXTILE_UNIT, TEXTILE_LIGHT } from "./squareMath";
import { makeNoiseAtlas } from "./noiseAtlas";
import { OutlineOverlay, type OutlineItem } from "./outlineOverlay";
import { BlueprintOverlay, type BlueprintTile } from "./blueprintOverlay";
import { NOISE_FIELDS } from "./material";
import { LIGHT_QUANT } from "./shadowGather";
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
  /** Flat-normal / zero-dir fallback (0.5,0.5,1.0) — neutral relief when no normal/dir map is bound. */
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
  /** lighting-feel P3: AO strength on the blit's ambient term (`__ao`; 0 = off). */
  aoStrength = 1;
  /** lighting-feel P3: emissive (self-lit) strength (`__emissive`; 0 = off). */
  emissiveBoost = 1.4;
  /** The `/overlayRT` debug material — draws one G-buffer composite over the display. */
  private readonly overlayShader: OverlayShader;
  /** The composite the overlay is currently showing (e.g. `normal-cold`), or null (off). */
  private overlayChannelName: string | null = null;
  /** Lights + the per-light shadow **bitfield** (gathered off the cold cache's standing prims). */
  private readonly shadows: ShadowGather;
  /** ui-select P1: the selection outline overlay (drawn topmost in {@link tick}). */
  private readonly outline: OutlineOverlay;
  /** build-walls P2: the blueprint drag-preview overlay (under the outline, over the world). */
  private readonly blueprint: BlueprintOverlay;
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
    this.shadows = new ShadowGather(this.renderer);
    this.outline = new OutlineOverlay(this.renderer, this.empty);
    this.blueprint = new BlueprintOverlay(this.renderer, this.empty);

    this.grid = new Program(gl, GRID_VERT, GRID_FRAG, "viewport-grid");
    this.gridQuad = new Geometry(gl, this.grid, {
      aPosition: { data: new Float32Array([-1, -1, 3, -1, -1, 3]), size: 2 },
    });

    // DEBUG (textile-slot P6): the viewport + its caches, so a zoom sweep can be asserted on DIRTY
    // COUNTS and map dims rather than screenshots. The scene is unlit by default, so sampling the
    // canvas cannot distinguish a re-bake flash from ordinary darkness — the cache state can.
    (globalThis as unknown as { __viewport: Viewport }).__viewport = this;
    // DEBUG (lighting-feel P3): AO strength on the ambient term — __ao(0) = off (the A/B), 1 = full.
    (globalThis as unknown as { __ao: (s?: number) => number }).__ao = (s?: number) => {
      if (s !== undefined) this.aoStrength = s;
      return this.aoStrength;
    };
    // DEBUG (material-system P4): global colour-placement override for the F1 by-eye A/B —
    // __material(0 uv | 1 world | 2 detail-keyed | 3 normal-keyed), no arg / -1 = per-material.
    (globalThis as unknown as { __material: (mode?: number) => number }).__material = (mode?: number) => {
      const m = mode ?? -1;
      this.map.setPlaceMode(m);
      this.warm.setPlaceMode(m);
      this.map.invalidateAll();
      this.warm.invalidateAll();
      return m;
    };
    // DEBUG (lighting-feel P3): emissive strength — __emissive(0) = off (the A/B).
    (globalThis as unknown as { __emissive: (b?: number) => number }).__emissive = (b?: number) => {
      if (b !== undefined) this.emissiveBoost = b;
      return this.emissiveBoost;
    };
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
          const solid = (residual: TexFrame) => ({ texture: white, tint: prim.geoColor ?? prim.tint, material: { residual, layers: null, surface: wf, chA: ZERO_CH, chB: ZERO_CH, chC: ZERO_CH } });
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
          let chC = ZERO_CH;
          const reg = this.materialRegistry;
          if (reg) {
            const lyr = r.resolve(prim.textureName, "layers", prim.cell);
            if (!lyr.geo && lyr.frame) {
              layers = lyr.frame;
              ({ chA, chB, chC } = reg.packChannels(prim.packed));
            }
          }
          return { texture: white, tint: 0xffffff, material: { residual: alb.frame, layers, surface: surf.frame, chA, chB, chC } };
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
      // #3 zdepth-world: encode the thing's BASE-Y ROW (front/behind key) so the lighting can skip
      // shadowing a sprite drawn in FRONT of the caster. B byte = 0 for ground (zIndex 0 → no depth test);
      // for a thing the high bit 0x80 flags "is a thing" and the low 7 bits carry the base row (mod 128).
      // MUST use the SAME row convention as the gather's caster depth: the draw-box bottom
      // `prim.y + prim.height` (= coldShadowData's stored base-centre `ay`), NOT zRow — those differ by a
      // per-sprite anchor offset, which biased the comparison (some trees right, some wrong).
      { key: `zdepth-world-${suffix}`, resolve: (prim) => ({ texture: white, tint: 0xffffff, depth: prim.zIndex >= 1 ? (0x80 | (Math.floor((prim.y + prim.height) / SQUARE) & 0x7f)) / 255 : -1 }) },
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
  /** Build + bind the tiling noise atlas the material bake samples (material-system P0 — the
   *  stub previously bound null, which zeroed every material's variation). Owns the GL context,
   *  so construction lives here; both tiers get it (movers may bind materials later). Re-bakes. */
  buildNoiseAtlas(): void {
    const atlas = makeNoiseAtlas(this.renderer.gl);
    this.map.setNoise(atlas, NOISE_FIELDS.length, 1, SQUARE * 2);
    this.warm.setNoise(atlas, NOISE_FIELDS.length, 1, SQUARE * 2);
    this.map.invalidateAll();
    this.warm.invalidateAll();
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

  /** The channels `/overlayRT` can show — the composites + the shadow bitfield (its own decode path). */
  overlayChannelNames(): string[] {
    return [...this.renderTextures().map((c) => c.name), "shadow-cold"];
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
  /** Toggle the shadow pass; returns whether it's now on. */
  toggleShadows(): boolean {
    return this.shadows.toggle();
  }
  /** Current LOGICAL zoom (the lod dial, not screen px per world px — that is `camera.renderScale`). */
  get zoom(): number {
    return this.camera.zoom;
  }
  /** The cold cache's tile window (`SLOTS << lod` tiles + its origin). The authoritative answer to
   *  "how much world will we draw", so the zone subscription sizes itself from this rather than
   *  re-deriving a screen estimate that can disagree with what the renderer actually bakes. */
  get window(): { winCol: number; winRow: number; cols: number; rows: number; slotPx: number; lod: number } {
    return this.map.window;
  }
  /** Set an absolute zoom, holding the viewport centre; returns the anchor to push
   *  through the bridge (so zone subscriptions follow), or null if it clamped to a no-op.
   *
   *  **Callers MUST push the returned anchor through `WorldBridge.zoomTo`.** This moves the CAMERA only —
   *  it does not update the bridge's own zoom, so the zone subscription would stay sized for the previous
   *  lod and the window edges would have no tile data. From a console use **`__zoom(z)`**, which does both
   *  (work `2026-07-26-textile-slot` I11 — this exact shortcut cost real debugging time). */
  setZoom(z: number): { x: number; y: number } | null {
    return this.camera.setZoom(z);
  }
  screenToWorld(sx: number, sy: number): { x: number; y: number } {
    return this.camera.screenToWorld(sx, sy);
  }

  /** ui-select P1: replace the outlined selection set (the scene rebuilds per frame). */
  setOutlines(items: OutlineItem[]): void {
    this.outline.set(items);
  }

  /** build-walls P2: replace the blueprint preview tiles (empty = clear). */
  setBlueprint(tiles: BlueprintTile[]): void {
    this.blueprint.set(tiles);
  }

  /** ui-select P1: a COLD standing prim by id (the warm sibling is {@link warmGetPrim}). */
  coldGetPrim(id: number): Primitive | null {
    return this.map.getPrim(id);
  }

  /** ui-select P0 (D2): the topmost COLD standing prim whose TIGHT silhouette box contains the
   *  world point — painter order (zIndex, then southernmost) picks among overlaps. Movers are
   *  the MoverLayer's to hit-test (they carry entity identity this cache doesn't know). */
  thingAt(wx: number, wy: number): { primId: number } | null {
    let best: { id: number; zIndex: number; y: number } | null = null;
    for (const p of this.map.standingPrims()) {
      const b = this.shadows.tightBoxFor(p, this.resolver);
      if (!b) continue;
      if (wx < b.x || wx >= b.x + b.w || wy < b.y || wy >= b.y + b.h) continue;
      if (!best || p.zIndex > best.zIndex || (p.zIndex === best.zIndex && p.y > best.y)) {
        best = { id: p.id, zIndex: p.zIndex, y: p.y };
      }
    }
    return best ? { primId: best.id } : null;
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
  /** build-walls P1: re-bake a COLD prim after an in-place field mutation (a linked tile's
   *  `cell` re-pick) — the cold sibling of {@link warmRefreshPrim}. */
  refreshPrim(id: number): void {
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
  /** hot-sync P1 (F1): the ONE hot dirty — a mover's visual change fans atomically into the
   *  warm-cache dirty (refreshPrim, called by the MoverLayer just before this) AND the hot
   *  light/shadow/receiver rects + record rewrite (ShadowGather.moverDirty), from the same
   *  position snapshot at the same eps crossing. */
  moverDirty(id: number): void {
    const prim = this.warm.getPrim(id);
    if (prim) this.shadows.moverDirty(prim, this.resolver);
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
    // LOGICAL zoom drives the lod ladder (monitor-independent — every player is on the same lod at the
    // same zoom); RENDER scale drives anything measured in actual screen px.
    const z = this.camera.zoom;
    const rs = this.camera.renderScale;
    const ax = this.camera.anchorX;
    const ay = this.camera.anchorY;
    // Aim texture loads at the ACTUAL on-screen tile size (a tile is SQUARE world px), so a large
    // display still asks for the sharpest art it can use. Clamped to SQUARE inside the resolver.
    this.resolver?.setTargetLod(SQUARE * rs);

    // Both caches share the window/slot geometry (identical inputs), so their composites stay
    // slot-aligned for the warm-over-cold display blit.
    this.warm.resize(w, h, z, ax, ay);
    this.map.resize(w, h, z, ax, ay);
    this.warm.recenter(ax, ay);
    this.map.recenter(ax, ay);
    // Warm bakes UNBUDGETED (hot-sync F4): movers are a handful and their sprite bake must
    // land the SAME frame as the lighting their dirty raised — a deferred sprite under fresh
    // lighting is exactly the desync hot-sync kills. The budget still governs cold streaming.
    this.warm.bakeDirty(Number.MAX_SAFE_INTEGER);
    this.map.bakeDirty(Math.max(BAKE_BUDGET - this.warm.lastBaked, COLD_BAKE_FLOOR));

    // Recompute the shadow bitfield + bake the LIGHTMAP (gather → RTs) BEFORE the display, so the blit
    // multiplies this frame's lighting. Renders into the shadow/light RTs; the blit below draws to screen.
    // pawn-render P2: WARM (mover) prims join the pass tagged hot — their records carry the class bit,
    // the shaders route every hot participant to the HOT maps only (the ratified tier matrix), so a
    // moving wolf never dirties a cold bake.
    if (this.map.ready) {
      this.shadows.tick([...this.map.standingPrims(), ...this.warm.standingPrims()], this.resolver, this.map.window);
    }

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
        // Lighting (#4): sum the COLD (static) + HOT (dynamic) lightmaps + ambient. Sampled by world
        // position via the cold window mapping (same toroidal tile grid as shadow-cold). Null → UNLIT
        // fallback. P2 also binds the aggregate light-dir maps + the cold/warm normal composites for relief.
        const coldLight = this.shadows.coldLightmap;
        this.blitShader.coldLight = coldLight;
        this.blitShader.hotLight = this.shadows.hotLightmap;
        this.blitShader.decay = this.shadows.decayMap; // lighting-feel P2: ephemeral particle glow
        // lighting-feel P3: the emissive mask rides the zdepth composites' R lane.
        this.blitShader.depth = this.map.displayComposite("zdepth-world-cold");
        this.blitShader.depthWarm = this.warm.displayComposite("zdepth-world-warm");
        const win = this.map.window;
        // world px → clip: x = (wx-ax)*2rs/w, y = -(wy-ay)*2rs/h  (screen y-down → clip y-up).
        // RENDER scale, not logical zoom — this is the transform that puts world px on the display, so
        // it carries the cover fit that makes the visible world identical on every monitor.
        const proj = new Float32Array([
          (2 * rs) / w, 0, 0,
          0, -(2 * rs) / h, 0,
          -ax * (2 * rs) / w, ay * (2 * rs) / h, 1,
        ]);
        this.renderer.draw({
          program: this.blitShader.program,
          geometry: this.displayGeo!,
          blend: "normal",
          textures: this.blitShader.textures(this.empty),
          uniforms: (p) => {
            p.uMat3("uProjection", proj);
            p.uInt("uLightEnable", coldLight ? 1 : 0);
            p.uFloat("uAmbient", AMBIENT_LEVEL);
            p.uInt("uLCols", win.cols);
            p.uInt("uLRows", win.rows);
            p.uInt("uLWinCol", win.winCol);
            p.uInt("uLWinRow", win.winRow);
            // The lightmap rides the fixed slot grid, so its per-TILE texel size is `TEXTILE_LIGHT >> lod`.
            // This USED to be `win.slotPx`, and that was correct only while `TEXTILE_SQUARE === SQUARE` made
            // the lightmap and the art maps the same resolution. They are now separate dials
            // (`2026-07-28-square-128`), so `win.slotPx` is the ART texel size and feeding it here samples
            // the lightmap at the wrong scale the moment the two differ — demonstrated in P0, which raised
            // only the lighting resolution and rendered visible per-slot misregistration.
            //
            // The general rule, twice re-learned (see `2026-07-24-map-compatibility`): a map's slot stride
            // comes from THAT MAP's own texels-per-tile constant, never from another map's.
            p.uInt("uLSlot", Math.max(1, TEXTILE_LIGHT >> win.lod));
            // lighting-feel P2: the decay map is COARSE — TEXTILE_UNIT texels/tile at lod 0,
            // halving with lod exactly like the shadow map (SHADOW_TEXELS >> lod).
            p.uInt("uDSlot", Math.max(1, TEXTILE_UNIT >> win.lod));
            p.uFloat("uAoStr", this.aoStrength);
            p.uFloat("uEmissiveBoost", this.emissiveBoost);
            p.uFloat("uLightQuant", LIGHT_QUANT); // de-quantise the additive accumulator (F11b)
          },
        });

        // Debug overlay (`/overlayRT`): draw one G-buffer composite over the lit world, in exact
        // register (same display geometry + `uProjection`). The fragment drops the channel's "empty"
        // value so the world reads through elsewhere.
        const ov = this.overlayChannelName;
        if (ov === "shadow-cold") {
          this.shadows.drawOverlay(this.camera, this.map.window); // its own bitfield decode path
        } else if (ov) {
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

    if (this.gridLevel > 0) {
      this.renderer.draw({
        program: this.grid,
        geometry: this.gridQuad,
        blend: "normal",
        uniforms: (p) => {
          p.uVec2("uViewport", w, h);
          p.uVec2("uAnchor", ax, ay);
          p.uFloat("uZoom", rs); // the grid overlays SCREEN px, so it wants the render scale
          p.uVec3("uPitch", SQUARE, SQUARE * ZONE_DIM, SQUARE * ZONE_DIM * REGION_DIM);
          p.uFloat("uLevel", this.gridLevel);
        },
      });
    }

    // build-walls P2: the blueprint drag preview, then ui-select P1's selection outlines
    // topmost (over grid/overlays — a selection must never hide).
    this.blueprint.draw(this.camera);
    this.outline.draw(this.camera);
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
