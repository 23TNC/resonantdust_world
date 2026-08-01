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
import { installReachCheck } from "./lightReach";
import { Records, INDEX_NONE, ROTATIONS_PER_DEF } from "./records";
import { LightPass } from "./lightPass";
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
  /** lighting-rework P3: per-light slots + the summed map the display reads. */
  readonly lights: LightPass;
  /** The synthetic emitter P2/P3 exercise the real path with (no lights are authored yet). */
  private debugLightPrim = 0;
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
    this.lights = new LightPass(this.renderer.gl);
    installFrameCost(this, this.renderer.gl);
    // lighting-rework P0 (F6): `__reachcheck()` — proves the TS and GLSL reach agree at all 1024 values.
    installReachCheck(this.renderer.gl);
    // lighting-rework P1: `__records()` — the record layer's own acceptance, run against the LIVE
    // scene rather than a fixture, so it proves the writers on real definitions and real positions.
    (globalThis as unknown as { __records: () => unknown }).__records = () => this.recordSelfTest();
    (globalThis as unknown as { __buildrecords: () => unknown }).__buildrecords = () => this.buildRecords();
    // lighting-rework P3: run the light pass and read back what it produced.
    (globalThis as unknown as { __lightpass: () => unknown }).__lightpass = () => this.runLightPass();
    // lighting-rework P3 item 5: cost per light, measured BEFORE shadows are built on top of it.
    (globalThis as unknown as { __lightcost: (n?: number) => unknown }).__lightcost =
      (n?: number) => this.lightCost(n ?? 8);
    // lighting-rework P3 item 4: add a light, remove it, and prove the sum returns bit-identically.
    (globalThis as unknown as { __lightexact: () => unknown }).__lightexact = () => this.lightExactness();
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
      intensityOver10Bits: threw(() => rec.writePrim(ids[0], { unitX: 0, unitY: 0, definition: bb, intensity: 1024 })),
      // NOT a throw by design: F7 clamps rotation at the writer, so an out-of-range value is
      // corrected (and warned about in dev) rather than rejected. Assert the CLAMP instead — the
      // point of the lane check is that nothing can reach the GPU too wide for its bits.
      rotationClampedNotThrown: (() => {
        rec.writePrim(ids[1], { unitX: 0, unitY: 0, definition: bb, rotation: 99 });
        return rec.debugPrim(ids[1]).rotation === rec.rotationsOf(bb) - 1;
      })(),
      unitOver16Bits: threw(() => rec.writePrim(ids[0], { unitX: 70000, unitY: 0, definition: bb })),
      spanZeroRejected: threw(() => rec.writeDefinition(bb, 0, { frameX: 0, frameY: 0, frameSpan: 0 })),
      validWriteSucceeds: !threw(() => rec.writePrim(ids[0], { unitX: 1600, unitY: 800, definition: bb, intensity: 1023 })),
    };

    // round-trip a real standing prim through the record and back
    const live = [...this.map.standingPrims()][0];
    if (live) {
      const id = rec.allocPrim();
      const ux = Math.round(live.x / (SQUARE / 16)), uy = Math.round(live.y / (SQUARE / 16));
      rec.writePrim(id, { unitX: ux & 0xffff, unitY: uy & 0xffff, definition: bb, rotation: 2, intensity: 512 });
      const back = rec.debugPrim(id);
      out.roundTrip = { wrote: { ux: ux & 0xffff, uy: uy & 0xffff, rotation: 2, intensity: 512 }, read: back,
                        exact: back.unitX === (ux & 0xffff) && back.unitY === (uy & 0xffff)
                               && back.rotation === 2 && back.intensity === 512 };
    }

    rec.upload(this.renderer.gl);
    out.stats = rec.stats;
    out.glError = this.renderer.gl.getError();
    return out;
  }

  /** lighting-rework P1 — mint records for every live standing prim, straight from the resolver.
   *
   *  This is what "the record layer is proven" actually needs: not that the G-buffer bake draws FROM
   *  records (it does not, and the design does not ask it to — the bake draws art, the lighting reads
   *  records), but that a record accurately locates its art in the atlas. A lighting pass holding a
   *  bare prim index must be able to find the caster's texture, and that is exactly what is checked. */
  buildRecords(): Record<string, unknown> {
    const rec = this.records, res = this.resolver;
    if (!res) return { error: "no resolver" };
    const U = SQUARE / 16;
    const byStem = new Map<string, number>();
    let minted = 0, wrote = 0, mismatched = 0;
    const placed = new Map<number, Primitive>();
    const samples: Record<string, unknown>[] = [];

    for (const p of this.map.standingPrims()) {
      const stem = p.textureName;
      if (!stem) continue;
      const frame = res.resolve(stem, "albedo", p.cell ?? 0)?.frame;
      if (!frame) continue;

      let block = byStem.get(stem);
      if (block === undefined) {
        block = rec.allocDefinition(4);                       // billboards: n/e/s/w
        byStem.set(stem, block);
        minted++;
        const spanTiles = Math.max(1, Math.round(frame.w / SQUARE));
        const bb = res.opaqueBBox(stem);
        for (let r = 0; r < 4; r++) {
          rec.writeDefinition(block, r, {
            frameX: Math.round(frame.x / U), frameY: Math.round(frame.y / U),
            frameSpan: Math.min(16, spanTiles),
            anchorX: 1, anchorY: 2,                            // bottom-centre, as casters use
            subX: bb ? Math.round(bb.fx * spanTiles * 16) : 0,
            subY: bb ? Math.round(bb.fy * spanTiles * 16) : 0,
            subW: Math.max(1, bb ? Math.round(bb.fw * spanTiles * 16) : spanTiles * 16),
            subH: Math.max(1, bb ? Math.round(bb.fh * spanTiles * 16) : spanTiles * 16),
            castType: 1, receiveType: 2,
          });
        }
      }

      const id = rec.allocPrim();
      rec.writePrim(id, {
        unitX: Math.round((p.x + p.width / 2) / U) & 0xffff,
        unitY: Math.round((p.y + p.height) / U) & 0xffff,
        definition: block, rotation: 0, castType: 1, receiveType: 2,
        seed: Math.floor((p.seed ?? 0) * 255) & 0xff,
      });
      placed.set(id, p);
      wrote++;

      // CROSS-CHECK: decode the record and confirm it points back at the resolver's own frame.
      const d = rec.debugDefinition(block, 0);
      const wantX = Math.round(frame.x / U), wantY = Math.round(frame.y / U);
      if (d.frameX !== wantX || d.frameY !== wantY) mismatched++;
      else if (samples.length < 3) samples.push({ stem, frameUnits: [d.frameX, d.frameY], span: d.frameSpan });
    }

    // ── P2: the per-tile records ────────────────────────────────────────────────────────────────
    // presence — receivers ON each tile, layer-sorted. Built from the same walk, so the record set
    // and the drawn set cannot describe different scenes.
    const byTile = new Map<number, { index: number; layer: number }[]>();
    for (const [id, p] of placed) {
      const tx = Math.floor((p.x + p.width / 2) / SQUARE), ty = Math.floor((p.y + p.height) / SQUARE);
      const key = ((tx & 0xffff) << 16) | (ty & 0xffff);
      let list = byTile.get(key);
      if (!list) byTile.set(key, (list = []));
      list.push({ index: id, layer: p.zIndex ?? 0 });
    }
    for (const [key, list] of byTile) {
      const tx = (key >> 16) & 0xffff, ty = key & 0xffff;
      rec.writePresence(tx, ty, INDEX_NONE, list);
    }

    // light — the reach relation, from reachFromIntensity (F6). No lights are authored in the
    // stripped scene, so mint a real EMITTER prim at the focus tile and register that. It must be a
    // genuine emitter: the slot shader re-checks `emit_type` before trusting a slot (F9), so
    // pointing the light record at an arbitrary prim index yields nothing — as it should.
    const lightPrim = rec.allocPrim();
    rec.writePrim(lightPrim, {
      unitX: 100 * 16 + 8, unitY: 50 * 16 + 8,
      definition: 1, emitType: 1, intensity: 1023, colors: [0, 0, 0, 180],
    });
    this.debugLightPrim = lightPrim;
    rec.buildLights([{ index: lightPrim, tileX: 100, tileY: 50, intensity: 1023 }]);

    rec.upload(this.renderer.gl);

    // Layer order is what the per-pixel pass depends on, so assert it rather than trust the sort.
    let unsorted = 0, checked = 0;
    for (const [key, list] of byTile) {
      if (list.length < 2) continue;
      const tx = (key >> 16) & 0xffff, ty = key & 0xffff;
      const slots = rec.slotsAt("presence", tx, ty).slice(1).filter((s) => s !== INDEX_NONE);
      const layers = slots.map((s) => rec.debugPrim(s).layer);
      for (let i = 1; i < layers.length; i++) if (layers[i] < layers[i - 1]) unsorted++;
      checked++;
    }

    // The live scene puts one receiver per tile, so the sort and the caps were never exercised above.
    // Force both: 12 receivers on one tile in DELIBERATELY reversed layer order.
    const before = { recv: rec.droppedReceivers, lights: rec.droppedLights };
    const stack = Array.from({ length: 12 }, (_, i) => {
      const id = rec.allocPrim();
      rec.writePrim(id, { unitX: 9000, unitY: 9000, definition: 1, layer: 11 - i });
      return { index: id, layer: 11 - i };
    });
    rec.writePresence(200, 200, INDEX_NONE, stack);
    const kept = rec.slotsAt("presence", 200, 200).slice(1).filter((s) => s !== INDEX_NONE);
    const keptLayers = kept.map((s) => rec.debugPrim(s).layer);
    const overSubscribed = {
      offered: stack.length, keptSlots: kept.length,
      layers: keptLayers,
      ascending: keptLayers.every((v, i, a) => i === 0 || v >= a[i - 1]),
      keptTheTopmost: keptLayers[keptLayers.length - 1] === 11,
      droppedReceiversDelta: rec.droppedReceivers - before.recv,
    };
    // and over-subscribe the light cap: 10 lights on one tile, 8 slots
    rec.buildLights(Array.from({ length: 10 }, (_, i) => (
      { index: rec.allocPrim(), tileX: 210, tileY: 210, intensity: 200 })));
    const overLights = { droppedLightsDelta: rec.droppedLights - before.lights,
                         slotsUsed: rec.slotsAt("light", 210, 210).filter((s) => s !== INDEX_NONE).length };
    rec.upload(this.renderer.gl);

    const centre = rec.slotsAt("light", 100, 50).filter((s) => s !== INDEX_NONE).length;
    const edge = rec.slotsAt("light", 100 + 16, 50).filter((s) => s !== INDEX_NONE).length;
    const beyond = rec.slotsAt("light", 100 + 17, 50).filter((s) => s !== INDEX_NONE).length;

    return { definitionsMinted: minted, primsWritten: wrote, frameMismatches: mismatched, samples,
             presence: { tiles: byTile.size, multiReceiverTilesChecked: checked, outOfOrder: unsorted },
             overSubscribed, overLights,
             lightReach: { atCentre: centre, at16Tiles: edge, at17Tiles: beyond,
                           reachTilesForFullIntensity: 16 },
             stats: rec.stats, glError: this.renderer.gl.getError() };
  }

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
    const lights: { index: number; tileX: number; tileY: number; intensity: number }[] = [];
    for (let i = 0; i < n; i++) {
      const tx = win.winCol + 4 + (i % 8) * 3;
      const ty = win.winRow + 4 + Math.floor(i / 8) * 3;
      const id = rec.allocPrim();
      rec.writePrim(id, { unitX: tx * 16 + 8, unitY: ty * 16 + 8, definition: 1,
                          emitType: 1, intensity: 1023, colors: [0, 0, 0, 180] });
      lights.push({ index: id, tileX: tx, tileY: ty, intensity: 1023 });
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
                        definition: 1, emitType: 1, intensity: 900, colors: [0, 0, 0, 200] });
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
