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
import { PACKED_CHANNELS } from "./mrtBakeShader";
import type { MaterialRegistry } from "./material";
import { SQUARE, ZONE_DIM, REGION_DIM } from "./squareMath";
import { makeNoiseAtlas } from "./noiseAtlas";
import { OutlineOverlay, type OutlineItem } from "./outlineOverlay";
import { installFrameCost } from "./frameCost";
import { Records, INDEX_NONE, ROTATIONS_PER_DEF } from "./records";
import { LightPass } from "./lightPass";
import { ShadowBuffer, pairSlot, SHADOW_LIGHTS } from "./shadowPass";
import { RecordSync } from "./recordSync";
import { LIGHT_READ_SCALE } from "./lightPass";
import { BlueprintOverlay, type BlueprintTile } from "./blueprintOverlay";
import { NOISE_FIELDS } from "./material";
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
  /** lighting-rework P1: the flat record layer — `prim_data` + `definition_data`. */
  readonly records: Records;
  private readonly recordSync: RecordSync;
  /** lighting-rework P3: per-light slots + the summed map the display reads. */
  readonly lights: LightPass;
  /** lighting-rework P4: the per-unit shadow buffer, ping-ponged. */
  readonly shadows: ShadowBuffer;
  /** The synthetic emitter P2/P3 exercise the real path with (no lights are authored yet). */
  private debugLightPrim = 0;
  /** lighting-rework P5: is the new lighting driven + displayed? Off until `__lit(true)`. */
  // lighting-correctness P1b: LIT IS THE DEFAULT — the reconciler keeps records live, so the
  // chain simply runs once the map is ready. `__lit(false)` remains the unlit A/B.
  private litEnabled = true;
  /** lighting-rework I11: the live light set, and their orbit origins. The headline acceptance is
   *  MOVING lights, and a static measurement is the best case of the very mechanism it stresses —
   *  with nothing moving, last frame's caster is still the caster and the incumbent tier absorbs
   *  everything. `__orbit(true)` displaces every light each frame through the real record path. */
  private liveLights: { index: number; homeX: number; homeY: number; tileX: number; tileY: number }[] = [];
  private orbitOn = false;
  private orbitPhase = 0;
  /** lighting-rework P6: the gather's tier split + the refine gate's selectivity, for the debug
   *  panel's Lighting tab. Refreshed by `__gather()`; the panel shows "run __gather()" until then, so
   *  it never displays a stale number as if it were live. */
  lightingTiers: { incumbent: number; adjacency: number; walk: number; gatePct: number } | null = null;
  /** The `/overlayRT` debug material — draws one G-buffer composite over the display. */
  private readonly overlayShader: OverlayShader;
  /** The composite the overlay is currently showing (e.g. `normal-cold`), or null (off). */
  private overlayChannelName: string | null = null;
  /** Lights + the per-light shadow **bitfield** (gathered off the cold cache's standing prims). */
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
    // lighting-rework P0: `__framecost()` — the stream's ONE measurement instrument.
    this.records = new Records(this.renderer.gl);
    this.recordSync = new RecordSync(this.records);
    this.lights = new LightPass(this.renderer.gl);
    this.shadows = new ShadowBuffer(this.renderer.gl);
    installFrameCost(this, this.renderer.gl);
    // lighting-rework P1: `__records()` — the record layer's own acceptance, run against the LIVE
    // scene rather than a fixture, so it proves the writers on real definitions and real positions.
    (globalThis as unknown as { __records: () => unknown }).__records = () => this.recordSelfTest();
    (globalThis as unknown as { __buildrecords: () => unknown }).__buildrecords = () => this.recordSync.stats;
    // lighting-rework P3: run the light pass and read back what it produced.
    (globalThis as unknown as { __lightpass: () => unknown }).__lightpass = () => this.runLightPass();
    // lighting-rework P3 item 5: cost per light, measured BEFORE shadows are built on top of it.
    (globalThis as unknown as { __lightcost: (n?: number) => unknown }).__lightcost =
      (n?: number) => this.lightCost(n ?? 8);
    // lighting-rework P3 item 4: add a light, remove it, and prove the sum returns bit-identically.
    (globalThis as unknown as { __lightexact: () => unknown }).__lightexact = () => this.lightExactness();
    // lighting-rework P4: the shadow buffer's shape, its zeroed first frame, and the l >= 4 split.
    (globalThis as unknown as { __shadowbuf: () => unknown }).__shadowbuf = () => this.shadowBufferCheck();
    // lighting-rework P4: run the gather and histogram which TIER answered each (unit, light).
    (globalThis as unknown as { __gather: () => unknown }).__gather = () => this.runGather();
    // lighting-rework P5: drive the whole chain each frame and show it.
    (globalThis as unknown as { __receivers: () => unknown }).__receivers = () => this.receiverCheck();
    // lighting-rework I11: place N lights, and move them.
    (globalThis as unknown as { __lights: (n?: number) => unknown }).__lights = (n?: number) => this.placeLights(n ?? 16);
    (globalThis as unknown as { __orbit: (on?: boolean) => unknown }).__orbit = (on?: boolean) => {
      this.orbitOn = on ?? !this.orbitOn;
      return { orbiting: this.orbitOn, lights: this.liveLights.length };
    };
    (globalThis as unknown as { __lit: (on?: boolean) => unknown }).__lit = (on?: boolean) => {
      this.litEnabled = on ?? !this.litEnabled;
      return { lit: this.litEnabled };
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

  /** Whether the serving manifest lists a texture `name` (human-pawns P3) — MoverLayer's
   *  variant-folder fallback probe. False until the resolver attaches. */
  hasTexture(name: string): boolean {
    return this.resolver?.has(name) ?? false;
  }

  /** pawn-part-placement F4: register a stem's pre-atlas sprite scale + pivot. A part slot's
   *  authored `scale` reaches the resolver through here, so the art shrinks INSIDE its pow2 frame
   *  and every co-packed map (albedo/normal/surface) plus the opaque bbox — and therefore the
   *  shadow card — scales as one. A no-op before the resolver attaches; the MoverLayer re-registers
   *  on every visual apply, so the value lands as soon as it does. */
  setSpriteScale(stem: string, sw: number, sh: number, px: number, py: number): void {
    this.resolver?.setSpriteScale(stem, sw, sh, px, py);
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
      // P1b: frames moved (a lod landed / a page repacked) — every definition re-writes from
      // the resolver's CURRENT frames on the next reconcile (I2's staleness half).
      this.recordSync.markDefsStale();
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

  /** The channels `/overlayRT` can show — the G-buffer composites. lighting-strip P2 dropped
   *  `shadow-cold`: it named a render target that no longer exists, so offering it would have been a
   *  command that silently shows nothing. */
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

  /** ui-select P0 (D2): a standing prim's TIGHT silhouette box in world px — the hit-test target,
   *  so clicking a conifer's transparent corner does not select it.
   *
   *  lighting-strip P2 ([D2](../../../../docs/work/2026-07-31-lighting-strip/deviations.md)): this used
   *  to route through `ShadowGather.tightBoxFor` -> `ColdShadowData.tightBoxOf`, which was the ONLY
   *  non-lighting reader of the record layer. The resolver has held the same datum all along —
   *  `opaqueBBox` returns the silhouette as frame fractions, which the record path then quantised to
   *  EVEN units for the shadow card. Reading the fraction directly is both simpler and *finer*: hit
   *  testing has no reason to inherit the shadow card's 2-unit grid.
   *
   *  No bbox (unresolved art) falls back to the full quad — selectable, just not silhouette-tight. */
  private tightBoxFor(prim: Primitive): { x: number; y: number; w: number; h: number } | null {
    const bb = this.resolver?.opaqueBBox(prim.textureName) ?? null;
    if (!bb) return { x: prim.x, y: prim.y, w: prim.width, h: prim.height };
    const w = bb.fw * prim.width, h = bb.fh * prim.height;
    const dx0 = bb.fx * prim.width, dy = bb.fy * prim.height;
    // flipX mirrors the frame, so the opaque region reflects about the quad's centre line.
    const dx = prim.flipX ? prim.width - (dx0 + w) : dx0;
    return { x: prim.x + dx, y: prim.y + dy, w, h };
  }

  /** ui-select P0 (D2): the topmost COLD standing prim whose TIGHT silhouette box contains the
   *  world point — painter order (zIndex, then southernmost) picks among overlaps. Movers are
   *  the MoverLayer's to hit-test (they carry entity identity this cache doesn't know). */
  thingAt(wx: number, wy: number): { primId: number } | null {
    let best: { id: number; zIndex: number; y: number } | null = null;
    for (const p of this.map.standingPrims()) {
      const b = this.tightBoxFor(p);
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

    // lighting-rework P5: the new chain — gather (per unit) then the slot pass (per lighting texel,
    // with the gated refine), both bounded by the STORED reach lane (lighting-correctness P1).
    if (this.litEnabled && this.map.ready) {
      this.stepOrbit();
      // P1b: reconcile the records against the LIVE scene (both caches, movers included,
      // content lights from Primitive.light) — the seam I1 pinned as missing.
      this.recordSync.sync(this.renderer.gl, this.resolver, this.map.standingPrims(), this.warm.standingPrims());
      const w = this.map.window, r = this.records;
      const atlas = this.surfaceAtlas();
      // No atlas means the silhouette cannot be sampled. Skip the refine rather than run it against
      // a texture that answers 0 everywhere, which reads on screen as "the shadows are broken".
      const canRefine = atlas !== null;
      this.shadows.gather(this.renderer, { prim: r.primTex, def: r.defTex, light: r.lightTex,
                                           presence: r.presenceTex, atlas: atlas ?? this.white },
                          w.winCol, w.winRow);
      // P5: the receiver map FIRST, once -- it is geometry, so it is the same for every light and
      // the eight light fragments read it instead of each re-deciding (I2).
      this.lights.receivers(this.renderer, r.primTex, r.defTex, r.presenceTex, w.winCol, w.winRow);
      this.lights.run(this.renderer, r.primTex, r.lightTex, w.winCol, w.winRow,
                      { def: r.defTex, shadow: this.shadows.prev, atlas: atlas ?? this.white,
                        unitsX: 512, refine: canRefine });
    }
    // lighting-strip P1: the gather / lighting / receiver / decay passes are NO LONGER ISSUED. The
    // ShadowGather instance still exists and still compiles (P2 deletes it) -- this phase only stops
    // driving it, so the strip stays reversible while the unlit render is verified.

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
        // lighting-strip P1: no lightmap, decay or emissive bindings — the blit is UNLIT. The zdepth
        // composites stay: their B lane is the painter's key that resolves warm-over-cold per pixel.
        this.blitShader.lightmap = this.litEnabled ? this.lights.lightmap : null;
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
            // lighting-rework P5: ONE lightmap fetch per pixel (F1). `uLit` stays 0 until
            // `__lit(true)` turns the new system on, so the unlit resting state is still the default.
            p.uInt("uLit", this.litEnabled ? 1 : 0);
            p.uFloat("uLightRead", LIGHT_READ_SCALE);
            p.uFloat("uAmbient", 0.12);
            p.uInt("uLCols", win.cols); p.uInt("uLRows", win.rows);
            p.uInt("uLWinCol", win.winCol); p.uInt("uLWinRow", win.winRow);
            p.uInt("uLSlot", 64);
          },
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
    // lighting-strip P1: the preview used to sample the live lightmaps so a blueprint dragged beside
    // a torch glowed with it. There are no lightmaps now, so it draws unlit like everything else.
    this.blueprint.draw(this.camera, null);
    this.outline.draw(this.camera);
  }

  /** lighting-rework P1 — build records from the live scene and check every P1 acceptance.
   *
   *  Deliberately reads the SAME resolver the G-buffer bake draws through, so "the record says where
   *  the art is" is checked against where the art actually is, not against a second guess. */
  private recordSelfTest(): Record<string, unknown> {
    const rec = this.records;
    const out: Record<string, unknown> = {};

    // (1) allocation never yields the sentinel
    const ids = Array.from({ length: 64 }, () => rec.allocPrim());
    out.sentinelNeverAllocated = !ids.includes(INDEX_NONE) && ids[0] !== 0;

    // (3) base + rotation, for a 4-rotation billboard AND a 16-cell linked tile — no special case
    const bb = rec.allocDefinition(4);
    for (let r = 0; r < 4; r++) rec.writeDefinition(bb, r, { frameX: 10 + r, frameY: 20, frameSpan: 1 });
    const linked = rec.allocDefinition(ROTATIONS_PER_DEF);
    for (let r = 0; r < ROTATIONS_PER_DEF; r++) rec.writeDefinition(linked, r, { frameX: 100, frameY: 200 + r, frameSpan: 4 });
    out.billboardRotations = [0, 1, 2, 3].map((r) => rec.debugDefinition(bb, r).frameX);
    out.linkedRotations = [0, 7, 15].map((r) => rec.debugDefinition(linked, r).frameY);
    out.spanUnbiases = rec.debugDefinition(linked, 0).frameSpan === 4;

    // (4) a rotation past the allocation is CLAMPED at the writer, never read off the neighbour
    out.rotationClamp = { asked: 9, got: rec.clampRotation(bb, 9), allocated: rec.rotationsOf(bb) };

    // (5) lanes assert instead of truncating
    const threw = (fn: () => void): boolean => { try { fn(); return false; } catch { return true; } };
    out.laneAssertions = {
      intensityOver6Bits: threw(() => rec.writePrim(ids[0], { unitX: 0, unitY: 0, definition: bb, intensity: 64 })),
      reachOver16Rejected: threw(() => rec.writePrim(ids[0], { unitX: 0, unitY: 0, definition: bb, reach: 17 })),
      // NOT a throw by design: F7 clamps rotation at the writer, so an out-of-range value is
      // corrected (and warned about in dev) rather than rejected. Assert the CLAMP instead — the
      // point of the lane check is that nothing can reach the GPU too wide for its bits.
      rotationClampedNotThrown: (() => {
        rec.writePrim(ids[1], { unitX: 0, unitY: 0, definition: bb, rotation: 99 });
        return rec.debugPrim(ids[1]).rotation === rec.rotationsOf(bb) - 1;
      })(),
      unitOver16Bits: threw(() => rec.writePrim(ids[0], { unitX: 70000, unitY: 0, definition: bb })),
      spanZeroRejected: threw(() => rec.writeDefinition(bb, 0, { frameX: 0, frameY: 0, frameSpan: 0 })),
      validWriteSucceeds: !threw(() => rec.writePrim(ids[0], { unitX: 1600, unitY: 800, definition: bb, intensity: 63, reach: 16 })),
    };

    // round-trip a real standing prim through the record and back
    const live = [...this.map.standingPrims()][0];
    if (live) {
      const id = rec.allocPrim();
      const ux = Math.round(live.x / (SQUARE / 16)), uy = Math.round(live.y / (SQUARE / 16));
      rec.writePrim(id, { unitX: ux & 0xffff, unitY: uy & 0xffff, definition: bb, rotation: 2, intensity: 40, reach: 12 });
      const back = rec.debugPrim(id);
      out.roundTrip = { wrote: { ux: ux & 0xffff, uy: uy & 0xffff, rotation: 2, intensity: 40, reach: 12 }, read: back,
                        exact: back.unitX === (ux & 0xffff) && back.unitY === (uy & 0xffff)
                               && back.rotation === 2 && back.intensity === 40 && back.reach === 12 };
    }

    rec.upload(this.renderer.gl);
    out.stats = rec.stats;
    out.glError = this.renderer.gl.getError();
    return out;
  }

  /** lighting-rework P1 — mint records for every live standing prim, straight from the resolver.

  /** lighting-rework P3 — run the slot pass + the sum, then read back enough to prove both. */
  private runLightPass(): Record<string, unknown> {
    const gl = this.renderer.gl;
    const win = this.map.window;
    const rec = this.records;

    let draws = 0;
    const de = gl.drawElements, da = gl.drawArrays;
    gl.drawElements = function (...a: unknown[]) { draws++; return (de as (...x: unknown[]) => void).apply(gl, a); } as typeof gl.drawElements;
    gl.drawArrays = function (...a: unknown[]) { draws++; return (da as (...x: unknown[]) => void).apply(gl, a); } as typeof gl.drawArrays;
    this.lights.run(this.renderer, rec.primTex, rec.lightTex, win.winCol, win.winRow);
    gl.drawElements = de; gl.drawArrays = da;

    // read the SUM at the light's own tile and at the reach boundary
    const { w, h, slots } = this.lights.size;
    const texel = (tileX: number, tileY: number): [number, number] => [
      ((tileX - win.winCol) * 64 + 32), ((tileY - win.winRow) * 64 + 32),
    ];
    const readSum = (tx: number, ty: number): number[] => {
      const [px, py] = texel(tx, ty);
      if (px < 0 || py < 0 || px >= w || py >= h) return [-1, -1, -1];
      const out = new Float32Array(4);
      this.sumRTBind(gl);
      gl.readPixels(px, py, 1, 1, gl.RGBA, gl.FLOAT, out);
      gl.bindFramebuffer(gl.FRAMEBUFFER, null);
      return [+out[0].toFixed(4), +out[1].toFixed(4), +out[2].toFixed(4)];
    };

    return {
      drawsForAllLights: draws,
      lightPrim: this.debugLightPrim,
      lightRecord: this.debugLightPrim ? rec.debugPrim(this.debugLightPrim) : null,
      slotMap: { w: w * slots, h, slotsPerTexel: slots },
      sumAtLightTile: readSum(100, 50),
      sumAt8Tiles: readSum(108, 50),
      sumAt20Tiles: readSum(120, 50),
      glError: gl.getError(),
    };
  }

  private sumRTBind(gl: WebGL2RenderingContext): void {
    gl.bindFramebuffer(gl.FRAMEBUFFER, (this.lights.sumRT as unknown as { fbo: WebGLFramebuffer }).fbo);
  }

  /** lighting-rework P3 — the cost of N lights, on the same instrument everything else uses.
   *
   *  Taken now, before the shadow gather and the per-pixel refine exist, because this is the pass the
   *  plan flagged as the one cost nobody had bounded — and the old stream's mistake was building the
   *  expensive part first and pricing it afterwards. */
  private lightCost(n: number): Record<string, unknown> {
    const gl = this.renderer.gl, rec = this.records, win = this.map.window;
    // N emitters spread across the window so their reach circles genuinely overlap the sampled tiles
    const lights: { index: number; tileX: number; tileY: number; reach: number }[] = [];
    for (let i = 0; i < n; i++) {
      const tx = win.winCol + 4 + (i % 8) * 3;
      const ty = win.winRow + 4 + Math.floor(i / 8) * 3;
      const id = rec.allocPrim();
      rec.writePrim(id, { unitX: tx * 16 + 8, unitY: ty * 16 + 8, definition: 1,
                          emitType: 1, intensity: 63, reach: 16, colors: [0, 0, 0, 180] });
      lights.push({ index: id, tileX: tx, tileY: ty, reach: 16 });
    }
    rec.buildLights(lights);
    rec.upload(gl);

    const run = (): number => {
      for (let i = 0; i < 8; i++) this.lights.run(this.renderer, rec.primTex, rec.lightTex, win.winCol, win.winRow);
      gl.finish();
      const t0 = performance.now();
      for (let i = 0; i < 30; i++) this.lights.run(this.renderer, rec.primTex, rec.lightTex, win.winCol, win.winRow);
      gl.finish();
      return (performance.now() - t0) / 30;
    };
    const reps: number[] = [];
    for (let i = 0; i < 5; i++) reps.push(run());
    reps.sort((a, b) => a - b);
    const r = (x: number): number => Math.round(x * 1000) / 1000;
    return { lights: n, msPerLightingUpdate: r(reps[2]), min: r(reps[0]), max: r(reps[4]),
             droppedLights: rec.droppedLights, glError: gl.getError() };
  }

  /** lighting-rework P3 — the property the old accumulator needed `LIGHT_QUANT` to fake.
   *
   *  Add a light through the delta path, then remove it, and the sum must land BIT-IDENTICALLY back
   *  where it started. With per-light slots this is structural, not arranged: removal emits `-old`
   *  where `old` is the exact value the slot holds, so the two deltas are exact negatives. */
  private lightExactness(): Record<string, unknown> {
    const gl = this.renderer.gl, rec = this.records, win = this.map.window;
    const W = this.lights.size.w, H = this.lights.size.h;
    const snap = (): Float32Array => {
      const buf = new Float32Array(64 * 64 * 4);
      gl.bindFramebuffer(gl.FRAMEBUFFER, (this.lights.sumRT as unknown as { fbo: WebGLFramebuffer }).fbo);
      gl.readPixels((W >> 1) - 32, (H >> 1) - 32, 64, 64, gl.RGBA, gl.FLOAT, buf);
      gl.bindFramebuffer(gl.FRAMEBUFFER, null);
      return buf;
    };
    const diff = (a: Float32Array, b: Float32Array): { differing: number; worst: number } => {
      let n = 0, worst = 0;
      for (let i = 0; i < a.length; i++) {
        const d = Math.abs(a[i] - b[i]);
        if (a[i] !== b[i]) { n++; if (d > worst) worst = d; }
      }
      return { differing: n, worst };
    };

    // baseline: a full recompute, then snapshot
    this.lights.run(this.renderer, rec.primTex, rec.lightTex, win.winCol, win.winRow);
    const before = snap();

    // ADD a light into slot 7 (unused by the built set) via the delta path
    const id = rec.allocPrim();
    rec.writePrim(id, { unitX: (win.winCol + 16) * 16, unitY: (win.winRow + 8) * 16,
                        definition: 0, emitType: 1, intensity: 55, reach: 16, colors: [0, 0, 0, 200] });
    rec.upload(gl);
    let draws = 0;
    const de = gl.drawElements, da = gl.drawArrays;
    gl.drawElements = function (...a: unknown[]) { draws++; return (de as (...x: unknown[]) => void).apply(gl, a); } as typeof gl.drawElements;
    gl.drawArrays = function (...a: unknown[]) { draws++; return (da as (...x: unknown[]) => void).apply(gl, a); } as typeof gl.drawArrays;
    this.lights.updateLight(this.renderer, rec.primTex, 7, id, win.winCol, win.winRow);
    const drawsToAdd = draws;
    const added = snap();

    // REMOVE it — primIndex 0
    this.lights.updateLight(this.renderer, rec.primTex, 7, 0, win.winCol, win.winRow);
    gl.drawElements = de; gl.drawArrays = da;
    const after = snap();

    const changedByAdd = diff(before, added);
    const returned = diff(before, after);
    return {
      drawsPerLightUpdate: drawsToAdd,
      addChangedTexels: changedByAdd.differing,
      afterRemoveDifferingFloats: returned.differing,
      afterRemoveWorstDelta: returned.worst,
      bitIdentical: returned.differing === 0,
      glError: gl.getError(),
    };
  }

  /** lighting-rework P4 — the shadow buffer's acceptances: sized per UNIT, zeroed, and the slot
   *  split that the design's `l > 4` gets wrong. */
  private shadowBufferCheck(): Record<string, unknown> {
    const gl = this.renderer.gl;
    const d = this.shadows.dims;

    // read the CURRENT buffer back: every texel must be the 0 sentinel on a fresh allocation
    const sample = new Uint32Array(64 * 4);
    gl.bindFramebuffer(gl.FRAMEBUFFER, (this.shadows.cur as unknown as { fbo: WebGLFramebuffer }).fbo);
    gl.readPixels(0, 0, 64, 1, gl.RGBA_INTEGER, gl.UNSIGNED_INT, sample);
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    const nonZero = sample.reduce((n, v) => n + (v !== 0 ? 1 : 0), 0);

    // the l >= 4 split: all 8 lights must land on DISTINCT (px, slot) pairs, none past slot 7
    const pairs = Array.from({ length: SHADOW_LIGHTS }, (_, l) => ({ l, ...pairSlot(l) }));
    const keys = new Set(pairs.map((p) => `${p.px}:${p.slot}`));
    // what the design's `l > 4` would have produced, for the record
    const buggy = Array.from({ length: SHADOW_LIGHTS }, (_, l) =>
      l > 4 ? { l, px: 2, slot: (l - 4) * 2 } : { l, px: 1, slot: l * 2 });

    return {
      dims: d,
      megabytes: +(this.shadows.bytes / (1024 * 1024)).toFixed(2),
      perUnitNotPerTile: d.w === d.unitsX * d.pxPerUnit,
      firstFrameNonZeroTexels: nonZero,
      slotSplit: pairs,
      allDistinct: keys.size === SHADOW_LIGHTS,
      noneOverflow: pairs.every((p) => p.slot >= 0 && p.slot <= 6),
      designsBuggySplitOverflows: buggy.filter((p) => p.slot > 6).map((p) => `light ${p.l} -> slot ${p.slot}`),
      glError: gl.getError(),
    };
  }

  /** lighting-rework P4 — run the gather, then re-run it in tier-debug mode and histogram the result.
   *
   *  The hit rates are the item's acceptance: "the walk runs only when both cheap paths miss", which
   *  is a claim about proportions and therefore has to be counted, not argued. */
  private runGather(): Record<string, unknown> {
    const gl = this.renderer.gl, rec = this.records, win = this.map.window;
    const tex = { prim: rec.primTex, def: rec.defTex, light: rec.lightTex, presence: rec.presenceTex,
                  atlas: this.surfaceAtlas() ?? this.white };

    let draws = 0;
    const de = gl.drawElements, da = gl.drawArrays;
    gl.drawElements = function (...a: unknown[]) { draws++; return (de as (...x: unknown[]) => void).apply(gl, a); } as typeof gl.drawElements;
    gl.drawArrays = function (...a: unknown[]) { draws++; return (da as (...x: unknown[]) => void).apply(gl, a); } as typeof gl.drawArrays;
    // two real gathers: the first fills the incumbents, the second is the steady state the tiers describe
    this.shadows.gather(this.renderer, tex, win.winCol, win.winRow);
    this.shadows.gather(this.renderer, tex, win.winCol, win.winRow);
    const drawsPerGather = draws / 2;
    gl.drawElements = de; gl.drawArrays = da;

    // tier pass — same shader, uDebugTier=1, does NOT swap, so it cannot disturb the steady state
    this.shadows.gather(this.renderer, tex, win.winCol, win.winRow, true);
    const d = this.shadows.dims;
    const W = 256, H = 64;
    const buf = new Uint32Array(W * H * 4);
    gl.bindFramebuffer(gl.FRAMEBUFFER, (this.shadows.cur as unknown as { fbo: WebGLFramebuffer }).fbo);
    gl.readPixels(0, 0, W, H, gl.RGBA_INTEGER, gl.UNSIGNED_INT, buf);
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);

    const tally = [0, 0, 0, 0];      // 0 = no light reached / no caster, 1..3 = the tier that answered
    for (let i = 0; i < W * H; i++) {
      const packed = buf[i * 4];
      for (let l = 0; l < 8; l++) tally[(packed >>> (l * 2)) & 3]++;
    }
    const answered = tally[1] + tally[2] + tally[3];
    const pct = (n: number): number => (answered ? +((n / answered) * 100).toFixed(1) : 0);
    // F5's gate: the share of (texel, light) pairs that hold a caster and therefore do refine work.
    this.lightingTiers = { incumbent: pct(tally[1]), adjacency: pct(tally[2]), walk: pct(tally[3]),
                           gatePct: +((answered / (W * H * 8)) * 100).toFixed(2) };

    // ── item 5: the three-tier walk vs an EXHAUSTIVE search over the same scene ──────────────────
    const readCur = (): Uint32Array => {
      const b = new Uint32Array(W * H * 4);
      gl.bindFramebuffer(gl.FRAMEBUFFER, (this.shadows.cur as unknown as { fbo: WebGLFramebuffer }).fbo);
      gl.readPixels(0, 0, W, H, gl.RGBA_INTEGER, gl.UNSIGNED_INT, b);
      gl.bindFramebuffer(gl.FRAMEBUFFER, null);
      return b;
    };
    this.shadows.gather(this.renderer, tex, win.winCol, win.winRow);      // steady state (swaps)
    this.shadows.gather(this.renderer, tex, win.winCol, win.winRow);
    const walked = readCur();
    this.shadows.gather(this.renderer, tex, win.winCol, win.winRow, false, true);   // brute, no swap
    const brute = readCur();
    // Compare OCCLUSION, not identity. Where several casters block the same ray, "which one" is
    // arbitrary: the walk takes the first along the ray, brute the first in scan order, and both are
    // complete answers. The invariant that matters -- and the one a shadow is drawn from -- is
    // whether the slot is occluded AT ALL. See D4.
    let differingIdentity = 0, differingOcclusion = 0;
    for (let i = 0; i < walked.length; i++) {
      if (walked[i] !== brute[i]) differingIdentity++;
      const wHi = walked[i] >>> 16, wLo = walked[i] & 0xffff;
      const bHi = brute[i] >>> 16, bLo = brute[i] & 0xffff;
      if ((wHi !== 0) !== (bHi !== 0)) differingOcclusion++;
      if ((wLo !== 0) !== (bLo !== 0)) differingOcclusion++;
    }
    const differing = differingOcclusion;

    // ── item 4: the caster's type lane is verified AT USE, not trusted from the stored id ─────────
    // Pick a caster from the CPU side, not by guessing which px a readback word came from: the
    // sampled span interleaves the unit's 3 px, so `word >> 16` is only a ground caster 1 time in 3.
    // ── item 4: the caster's type lane is verified AT USE (F9) ───────────────────────────────────
    // Decisive form: clear cast_type on EVERY prim. If the shader trusted the stored id, the
    // incumbents would keep casting; because it re-checks the lane, every shadow must vanish.
    // Count CASTER fields only. px 0 packs 8 casters; px 1/2 pack (caster, receiver) pairs, so
    // their LOW half is a receiver and stays non-zero whatever the casters do -- counting raw
    // non-zero words would never reach 0 and would look like a failed guard.
    const casterFields = (buf: Uint32Array): number => {
      let n = 0;
      for (let i = 0; i < W * H; i++) {
        const px = (i % W) % 3;
        for (let c = 0; c < 4; c++) {
          const w = buf[i * 4 + c];
          if ((w >>> 16) !== 0) n++;
          if (px === 0 && (w & 0xffff) !== 0) n++;
        }
      }
      return n;
    };
    const before = casterFields(brute);
    const saved: [number, number][] = [];
    for (let i = 1; i <= rec.stats.prims; i++) {
      const d = rec.debugPrim(i);
      if (d.castType === 0) continue;
      saved.push([i, d.castType]);
      rec.writePrim(i, { unitX: d.unitX, unitY: d.unitY, unitZ: d.unitZ, definition: d.definition,
                         castType: 0, receiveType: d.receiveType, emitType: d.emitType,
                         layer: d.layer, seed: d.seed, intensity: d.intensity, reach: d.reach });
    }
    rec.upload(gl);
    this.shadows.gather(this.renderer, tex, win.winCol, win.winRow, false, true);
    const afterCleared = casterFields(readCur());
    for (const [i, ct] of saved) {
      const d = rec.debugPrim(i);
      rec.writePrim(i, { unitX: d.unitX, unitY: d.unitY, unitZ: d.unitZ, definition: d.definition,
                         castType: ct, receiveType: d.receiveType, emitType: d.emitType,
                         layer: d.layer, seed: d.seed, intensity: d.intensity, reach: d.reach });
    }
    rec.upload(gl);
    const typeLane = { castersRestored: saved.length, nonZeroBefore: before,
                       nonZeroAfterAllCastTypesCleared: afterCleared,
                       everyShadowVanished: afterCleared === 0 };

    return {
      drawsPerGather,
      walkVsBrute: { occlusionDiffering: differingOcclusion, identityDiffering: differingIdentity,
                     slotsCompared: walked.length * 2 },
      walkMatchesBrute: differingOcclusion === 0,
      typeLaneCheckedAtUse: typeLane,
      shadowMap: { w: d.w, h: d.h, pxPerUnit: d.pxPerUnit },
      sampled: { unitLightPairs: W * H * 8 },
      tierCounts: { none: tally[0], incumbent: tally[1], adjacency: tally[2], corridorWalk: tally[3] },
      tierPercentOfAnswered: { incumbent: pct(tally[1]), adjacency: pct(tally[2]), corridorWalk: pct(tally[3]) },
      gateSelectivityPct: +((answered / (W * H * 8)) * 100).toFixed(2),
      glError: gl.getError(),
    };
  }

  /** lighting-rework P5 — the receiver map's acceptance ([I2](issues.md#i2)).
   *
   *  The map is written by ONE draw with no light input at all, so "the same texel resolves to the
   *  same receiver for every light" is true by construction rather than by testing all eight. What is
   *  worth testing is that COVERAGE decides and that layer order breaks ties the right way. */
  private receiverCheck(): Record<string, unknown> {
    const gl = this.renderer.gl, r = this.records, w = this.map.window;
    let draws = 0;
    const de = gl.drawElements, da = gl.drawArrays;
    gl.drawElements = function (...a: unknown[]) { draws++; return (de as (...x: unknown[]) => void).apply(gl, a); } as typeof gl.drawElements;
    gl.drawArrays = function (...a: unknown[]) { draws++; return (da as (...x: unknown[]) => void).apply(gl, a); } as typeof gl.drawArrays;
    this.lights.receivers(this.renderer, r.primTex, r.defTex, r.presenceTex, w.winCol, w.winRow);
    gl.drawElements = de; gl.drawArrays = da;

    // Read the WHOLE map. A corner sample covers only the first 8x4 tiles of the window, which can
    // easily hold no receivers at all -- and "0 covered" then looks like a broken pass.
    const W = this.lights.size.w, H = this.lights.size.h;
    const buf = new Uint32Array(W * H);
    gl.bindFramebuffer(gl.FRAMEBUFFER, (this.lights.receiverRT as unknown as { fbo: WebGLFramebuffer }).fbo);
    gl.readPixels(0, 0, W, H, gl.RED_INTEGER, gl.UNSIGNED_INT, buf);
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);

    const owned = new Map<number, number>();
    let covered = 0;
    for (const v of buf) { if (v !== 0) { covered++; owned.set(v, (owned.get(v) ?? 0) + 1); } }
    // every owner must actually BE a receiver — coverage decided, so nothing else can have got in
    let nonReceivers = 0;
    for (const id of owned.keys()) if (r.debugPrim(id).receiveType === 0) nonReceivers++;

    return {
      drawsForWholeMap: draws,
      lightIndependent: "by construction — the pass takes no light input",
      texelsSampled: W * H, texelsWithAReceiver: covered,
      distinctOwners: owned.size,
      ownersThatAreNotReceivers: nonReceivers,
      glError: gl.getError(),
    };
  }

  /** The shared co-packed atlas page — where the caster SILHOUETTE lives ([I12](issues.md#i12)).
   *
   *  Searches for a prim that actually RESOLVES. The first standing prim may have no `textureName`
   *  at all, and taking it blindly is what broke the silhouette test on its first run: the resolve
   *  returned null, the code fell back to the 1x1 white texture, and an out-of-range `texelFetch` on
   *  that returns **0** — so every silhouette test failed and shadows vanished entirely.
   *
   *  That fallback was commented as a "safe degradation to the old rectangle". It was the opposite:
   *  the safe direction is *more* shadow, and it produced none. Returning null instead lets the
   *  caller skip the refine rather than silently render a world with no shadows in it. */
  private surfaceAtlas(): Texture | null {
    for (const p of this.map.standingPrims()) {
      if (!p.textureName) continue;
      const f = this.resolver?.resolve(p.textureName, "surface", p.cell ?? 0)?.frame;
      if (f?.source) return f.source;
    }
    return null;
  }

  /** lighting-rework I11 — place N lights across the window and remember their orbit origins. */
  placeLights(n: number): Record<string, unknown> {
    const r = this.records, w = this.map.window, gl = this.renderer.gl;
    this.liveLights = [];
    for (let i = 0; i < n; i++) {
      const tileX = w.winCol + 3 + (i % 8) * 4;
      const tileY = w.winRow + 4 + Math.floor(i / 8) * 7;
      const index = r.allocPrim();
      const homeX = tileX * 16 + 8, homeY = tileY * 16 + 8;
      r.writePrim(index, { unitX: homeX, unitY: homeY, definition: 0, emitType: 1,
                           intensity: 63, reach: 16, colors: [0, 0, 0, 190] });
      this.liveLights.push({ index, homeX, homeY, tileX, tileY });
    }
    this.rebuildLightSet();
    r.upload(gl);
    return { placed: n, orbiting: this.orbitOn };
  }

  /** Displace every light around its origin and re-register — through the REAL record path, so the
   *  incumbent tier genuinely invalidates. That is the whole point of measuring motion (I11). */
  private stepOrbit(): void {
    if (!this.orbitOn || this.liveLights.length === 0) return;
    const r = this.records;
    this.orbitPhase += 0.06;
    const R = 12;                                   // units — under a tile, so a light stays in its room
    for (let i = 0; i < this.liveLights.length; i++) {
      const L = this.liveLights[i];
      const ph = this.orbitPhase + i * 0.7;         // per-light phase, so they do not move in lockstep
      const ux = Math.round(L.homeX + Math.cos(ph) * R);
      const uy = Math.round(L.homeY + Math.sin(ph) * R);
      r.writePrim(L.index, { unitX: ux & 0xffff, unitY: uy & 0xffff, definition: 0, emitType: 1,
                             intensity: 63, reach: 16, colors: [0, 0, 0, 190] });
      L.tileX = Math.floor(ux / 16); L.tileY = Math.floor(uy / 16);
    }
    this.rebuildLightSet();
    r.upload(this.renderer.gl);
  }

  private rebuildLightSet(): void {
    this.records.buildLights(this.liveLights.map((L) => (
      { index: L.index, tileX: L.tileX, tileY: L.tileY, reach: 16 })));
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
    this.map.destroy();
    this.warm.destroy();
    this.white.destroy();
    this.empty.destroy();
  }
}
