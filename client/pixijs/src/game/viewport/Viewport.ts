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

import { Buffer, BufferUsage, Container, Geometry, Graphics, Mesh, Texture } from "pixi.js";
import type { Renderer } from "pixi.js";
import { LayoutNode } from "../layout/LayoutNode";
import type { TextureResolver } from "../../textures";
import { ZOOM_MAX, ZOOM_MIN } from "../../textures/lod";
import type { RtChannel } from "../panels/rt/RtPanel";
import { SquareCache, type PrimitiveSpec, type ChannelSpec, type Primitive } from "./SquareCache";
import { NOISE_FIELDS, PACKED_UNIFORM_LEN, type MaterialRegistry } from "./material";
import { SQUARE, ZONE_DIM, REGION_DIM } from "./squareMath";
import { makeLightingShader, type LightingShader } from "./lightingShader";
import { LightRig } from "../lighting/LightRig";
import { ShadowPass, type ShadowCaster } from "./shadowPass";
import { makeLightingBakeShader } from "./lightingBakeShader";

/** Dirty squares baked per frame. A fresh window dirties its whole grid; the budget
 *  spreads that over a few frames so the first open never hitches. */
const BAKE_BUDGET = 128;

/** Minimum cold squares baked per frame regardless of warm load — so a warm whole-window
 *  flood (fresh window / LOD swap re-dirties every warm slot, mostly empty) can't starve the
 *  static world of bake budget. */
const COLD_BAKE_FLOOR = 64;

/** The wedge's ground angle (0 = flat / depth vertical, π/2 = standing / depth N–S). */
const GROUND_ANGLE = (65 * Math.PI) / 180;

/** The tight bounding box of a sprite's PRESENT pixels — the SURFACE map's B channel (coverage)
 *  at/above the hard threshold — as fractions 0..1 of the texture (x: 0 left → 1 right, y: 0 top →
 *  1 bottom). The shadow FLOORS derive from it: the E/W floor is the box's BOTTOM edge (`maxY`, the
 *  foot the sprite stands on), the N/S floor its LEFT-RIGHT CENTRE (`(minX+maxX)/2`, the vertical
 *  midline a rolled sprite pivots on). Pixi discards the CPU bitmap after upload, so we read it
 *  back through `extract`. Null on failure (caller falls back to the whole box); a fully-transparent
 *  sprite returns the whole 0..1 box. Computed once per stem and cached. */
interface SpriteBBox {
  minX: number;
  maxX: number;
  minY: number;
  maxY: number;
}
const FULL_BBOX: SpriteBBox = { minX: 0, maxX: 1, minY: 0, maxY: 1 };
function spriteBBox(renderer: Renderer, texture: Texture): SpriteBBox | null {
  try {
    const out = renderer.extract.pixels(texture) as { pixels: Uint8ClampedArray | Uint8Array; width: number; height: number };
    const { pixels, width, height } = out;
    if (!width || !height || !pixels) return null;
    let minX = width, maxX = -1, minY = height, maxY = -1;
    for (let y = 0; y < height; y++) {
      const row = y * width;
      for (let x = 0; x < width; x++) {
        if (pixels[(row + x) * 4 + 2] >= 250) {
          // B = coverage
          if (x < minX) minX = x;
          if (x > maxX) maxX = x;
          if (y < minY) minY = y;
          if (y > maxY) maxY = y;
        }
      }
    }
    if (maxX < 0) return FULL_BBOX; // fully transparent → no crop
    return { minX: minX / width, maxX: (maxX + 1) / width, minY: minY / height, maxY: (maxY + 1) / height };
  } catch {
    return null;
  }
}

/** Encode a prim's SUB-TILE ROW as its depth byte: `5 + (row % 251)` → the range 5..255, with 0..4
 *  RESERVED (0 = blank / no depth, so the reserved band itself signals "nothing here" — no extra
 *  flag needed). `row = floor(worldY / (SQUARE / 3))` splits each tile into 3 depth bands
 *  (top/center/bottom) so things sharing a tile still order front-to-back — same input for the
 *  shadow CASTER and the zdepth_world RECEIVER, so they compare in one space. Written into
 *  zdepth_world.B and zdepth_screen.G; the lighting compare recovers `row = value·255 − 5` and
 *  wraps over the 251-ring. 3 units/tile → the ring spans ~83 tiles (unambiguous to ~41). */
function depthUnit(worldY: number): number {
  const row = Math.floor(worldY / (SQUARE / 3)); // 3 sub-rows per tile: top/center/bottom
  return (5 + (((row % 251) + 251) % 251)) / 255; // 5..255; 0..4 reserved
}

/** The shadow billboard roll for a texture stem's FACING (its trailing `/<dir>` segment): a
 *  front/back sprite (`s`/`n`) rolls its card edge-on to cast E/W (±1, sign per s vs n — north
 *  rolls opposite south); a side (`e`, west = `e`+flip) or linked (`l`) sprite uses the standard
 *  billboard (0). See {@link ShadowCaster.nsRoll}. */
function nsRollFor(textureName: string): number {
  const dir = textureName.slice(textureName.lastIndexOf("/") + 1);
  return dir === "s" ? 1 : dir === "n" ? -1 : 0;
}

/** Zero material-channel params (no jitter) — shared by the albedo path when a prim has no
 *  varying materials (the reconstruction is then just the residual). Read-only. */
const ZERO_CH = new Float32Array(PACKED_UNIFORM_LEN);

export class Viewport extends LayoutNode {
  /** The COLD world cache — bakes the `*-cold` channels (albedo/normal/surface/zdepth-world)
   *  from the static prim index (tiles + cold things). World-space, dirty-updated. */
  private readonly map: SquareCache;

  /** The WARM cache — the SAME four channels (keyed `*-warm`) baked from the MOVER prim index
   *  (pawns). Driven with the identical window/slot geometry as {@link map}, so its composites
   *  are slot-aligned and the lighting pass composites warm OVER cold by warm `surface.B`. Baked
   *  at a HIGHER dirty priority than cold (movers update first). This replaces the bespoke
   *  overlay-sprite mover path — pawns now light + shadow like everything else. */
  private readonly warm: SquareCache;

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
  /** The cold-light lightmap bake material (lighting P1), enabled on the cold cache; packed each
   *  frame from the rig's cold lights, its output (`lightmap-cold`) sampled by the display. */
  private readonly lightBake = makeLightingBakeShader();
  /** Per-stem sprite bounding box (present pixels, fractions of the texture) — the shadow floors
   *  derive from it. Computed once from the surface pixels and reused every frame. */
  private readonly bboxCache = new Map<string, SpriteBBox>();
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

  /** A screen-space layer drawn ABOVE the baked world mesh — for dynamic overlays that
   *  don't go through the (lit, baked) prim cache: the mover layer's pawn markers. Kept on
   *  top via `zIndex` (the mesh is `0`). Consumers add display objects here and position
   *  them each frame with {@link worldToScreen}. */
  private readonly overlayContainer = new Container();

  /** Debug tile grid — a red gfx layer drawn straight over the composited world (in the
   *  screen-space {@link overlayContainer}, not through the baked/lit prim cache) that outlines
   *  every tile so cell boundaries read at a glance. Enabled by the `?grid` URL param via
   *  {@link setDebugGrid}; `null` (and undrawn) until then. Redrawn each frame in {@link tick}
   *  since the line positions follow the camera pan/zoom. */
  private gridGfx: Graphics | null = null;

  constructor(resolver: TextureResolver) {
    super();
    this.resolver = resolver;
    // Sort children by zIndex so the overlay stays above the (later-created) mesh.
    this.container.sortableChildren = true;
    this.overlayContainer.zIndex = 1;
    this.container.addChild(this.overlayContainer);
    // The albedo channel: bake each prim through the three-tier resolver — a named prim
    // gets the best texture on hand (master → preview → geo); an unnamed tint-rect keeps
    // its own texture. Colour by tier — geo shows the prim's silhouette colour
    // (`geoColor`, falling back to `tint`); a loaded tier shows `tint`. (A future normal
    // channel is another entry here with its own resolve + flat-up geo fallback.)
    // The four G-buffer channels, identical resolve logic for both tiers — only the key's
    // `-cold`/`-warm` suffix differs (which prim index the cache holds). Built twice.
    const chan = (suffix: string): ChannelSpec[] => [
      {
        key: `albedo-${suffix}`,
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
          // ALWAYS reconstruct from the `layers` map when the stem has one (every split sprite
          // does). No material-variation gate: `packChannels` fills unauthored channels with
          // per-channel default grays, so even a sprite with no authored tints reconstructs its
          // materials instead of baking the colour-stripped residual. A stem with no `layers`
          // map resolves geo → skipped (residual == the full albedo, one fewer texture pull).
          const reg = this.materialRegistry;
          let layers: Texture | null = null;
          let chA: Float32Array = ZERO_CH;
          let chB: Float32Array = ZERO_CH;
          if (reg) {
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
        key: `normal-${suffix}`,
        resolve: (prim) => {
          // Standing things: premultiply the normal (real LOD, or flat-up) by surface.B — the
          // soft coverage — so the flat background never clobbers, and the over-blend onto the
          // ground normal computes the α-lerp. Gate on the SAME real tier as albedo (both albedo
          // AND surface loaded) so a thing never bakes as real in one channel but geo in another.
          if (prim.zIndex >= 1 && prim.textureName) {
            const alb = resolver.resolve(prim.textureName, "albedo", prim.cell);
            const surf = resolver.resolve(prim.textureName, "surface", prim.cell);
            if (!alb.geo && !surf.geo) {
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
        key: `surface-${suffix}`,
        resolve: (prim) => {
          if (!prim.textureName) return { texture: resolver.white, tint: 0x00ffff };
          // Same real-tier gate as albedo: don't bake a thing's real surface (its coverage /
          // presence) until albedo is real too, else depth/normal (which read this) and the
          // occlusion see a thing albedo renders as a geo box.
          const alb = resolver.resolve(prim.textureName, "albedo", prim.cell);
          const { texture, geo } = resolver.resolve(prim.textureName, "surface", prim.cell);
          return alb.geo || geo ? { texture: resolver.white, tint: 0x00ffff } : { texture, tint: 0xffffff, surface: true };
        },
      },
      // The depth channel: B = the prim's tile-depth (row%256, /255), for STANDING things only —
      // ground/tiles + geo-tier bake nothing. Baked once, dirty-tracked, world-space; the
      // lighting pass reads B at vUV and omits a shadow where it's ≥ the caster depth. The
      // silhouette/presence comes from the SURFACE map's B (via the depth bake's smoothstep).
      {
        key: `zdepth-world-${suffix}`,
        resolve: (prim) => {
          // Same real-tier gate as albedo (both albedo AND surface loaded): a geo-tier thing
          // (albedo still a box) must NOT write a thing depth, or zdepth_world shows
          // thing-silhouettes that albedo/normal/surface don't.
          if (prim.zIndex >= 1 && prim.textureName) {
            const alb = resolver.resolve(prim.textureName, "albedo", prim.cell);
            const surf = resolver.resolve(prim.textureName, "surface", prim.cell);
            if (!alb.geo && !surf.geo) return { texture: surf.texture, tint: 0xffffff, depth: depthUnit(prim.y + prim.height) };
          }
          return { texture: resolver.white, tint: 0xffffff, depth: -1 }; // ground / geo → opaque black (no thing)
        },
      },
      // The cold LIGHTMAP (cold cache only): a DERIVED composite baked from the normal slot (the cold
      // lights summed, amortized). `resolve` is never called (derived); a placeholder keeps the type.
      ...(suffix === "cold"
        ? [{ key: "lightmap-cold", derived: true, resolve: () => ({ texture: resolver.white, tint: 0 }) } as ChannelSpec]
        : []),
    ];
    this.map = new SquareCache(chan("cold"));
    this.warm = new SquareCache(chan("warm"));
    // Cold-light lightmap bake (lighting P1) on the cold cache: the Viewport packs the rig's cold
    // lights onto it before each `bakeDirty`; the display samples `lightmap-cold`.
    this.map.enableLightBake(this.lightBake);
    // When a tier lands, re-dirty every prim'd square (both tiers) so the bake upgrades.
    this.unsubLoad = resolver.onLoad(() => {
      this.map.invalidateAll();
      this.warm.invalidateAll();
    });
  }

  /** Swap in the material registry (from the content bundle; re-set on hot-swap) and
   *  re-bake so the albedo picks up any newly-authored material variation. */
  setMaterialRegistry(registry: MaterialRegistry): void {
    this.materialRegistry = registry;
    this.map.invalidateAll();
    this.warm.invalidateAll();
  }

  /** Bind the tiling noise atlas the material bake samples. `rows` is the field count
   *  (atlas rows). A `null` texture disables the material path. Re-bakes so it takes. */
  setNoiseAtlas(texture: Texture | null, rows: number = NOISE_FIELDS.length): void {
    // uvTile = 1 noise tile across a sprite; worldTile = 2 world tiles (128px) per noise
    // tile, so ground mottle spans a couple of cells before repeating.
    this.map.setNoise(texture, rows, 1, SQUARE * 2);
    this.warm.setNoise(texture, rows, 1, SQUARE * 2);
    this.map.invalidateAll();
    this.warm.invalidateAll();
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

  /** Move an existing primitive to a new world-px top-left, updating the spatial
   *  index + dirty set. Cheaper than remove+add for a per-frame position tween —
   *  the interpolated free-thing movers call this every frame. No-op if the id is
   *  unknown or the position is unchanged. */
  movePrim(id: number, x: number, y: number): void {
    const prim = this.map.getPrim(id);
    if (!prim || (prim.x === x && prim.y === y)) return;
    prim.x = x;
    prim.y = y;
    this.map.refreshPrim(id);
  }

  // ── warm (mover) prim index ───────────────────────────────────────────────────
  /** Add a mover primitive to the WARM cache, returning its id. Pawns feed here instead of
   *  the (removed) overlay layer — they bake through the same albedo/normal/surface/depth
   *  channels as cold things and so light + shadow identically, just at warm dirty priority. */
  warmAddPrim(spec: PrimitiveSpec): number {
    return this.warm.addPrim(spec);
  }

  /** The mutable warm primitive for `id` (mutate its `x`/`y`/`textureName`/`cell`/`flipX`/
   *  `tint`, then call {@link warmRefreshPrim}), or null. Lets a mover update position AND
   *  facing in one place as the pawn walks/turns. */
  warmGetPrim(id: number): Primitive | null {
    return this.warm.getPrim(id);
  }

  /** Re-index + dirty a warm prim after mutating it (moved/turned/retextured). */
  warmRefreshPrim(id: number): void {
    this.warm.refreshPrim(id);
  }

  /** Remove a warm mover primitive. */
  warmRemovePrim(id: number): void {
    this.warm.removePrim(id);
  }

  /** The light rig — register world-space point lights (mutate them to move a torch), set
   *  the sun/ambient, or drive the cursor light. Lights are in WORLD px. */
  get lights(): LightRig {
    return this.rig;
  }

  /** The screen-space overlay layer above the baked world (see {@link overlayContainer}) —
   *  the mover layer parents its pawn markers here. */
  get overlay(): Container {
    return this.overlayContainer;
  }

  /** Toggle the red debug tile grid (the `?grid` URL param). Lazily builds the gfx layer on
   *  first enable; the actual lines are (re)drawn each frame in {@link tick} against the live
   *  camera. Disabling hides it (kept around, cheaply, for a later re-enable). */
  setDebugGrid(on: boolean): void {
    if (on && !this.gridGfx) {
      this.gridGfx = new Graphics();
      this.overlayContainer.addChild(this.gridGfx);
    }
    if (this.gridGfx) this.gridGfx.visible = on;
  }

  /** Redraw the debug grid for the current camera: three nested line sets, each on the
   *  boundaries of a world division — tiles (red, {@link SQUARE} px), zones (magenta,
   *  {@link ZONE_DIM} tiles) and regions (blue, {@link REGION_DIM}·{@link ZONE_DIM} tiles).
   *  Drawn in body-local screen px so it sits directly over the composited display, cleared +
   *  rebuilt each frame. Coarser sets are drawn LAST so their line wins where boundaries
   *  coincide (a region edge overdraws the zone + tile edge on the same pixel). No-op unless
   *  the grid is enabled. */
  private drawGrid(): void {
    const g = this.gridGfx;
    if (!g || !g.visible) return;
    g.clear();
    const w = this.width;
    const h = this.height;
    const z = this.zoomFactor;
    if (w <= 0 || h <= 0 || z <= 0) return;
    const zonePx = SQUARE * ZONE_DIM;
    const regionPx = zonePx * REGION_DIM;
    // Fine → coarse: the region lines paint over the zone + tile lines at shared boundaries.
    // Line weight scales with the division — thin tiles, heavier zones, heaviest regions.
    this.drawGridLines(g, SQUARE, 0xff0000, 1);
    this.drawGridLines(g, zonePx, 0xff00ff, 2);
    this.drawGridLines(g, regionPx, 0x0000ff, 3);
  }

  /** Stroke every `pitch`-px world boundary (vertical + horizontal) across the visible extent
   *  in `color` at `width` screen px, one `stroke()` per call. Screen-space (body-local px),
   *  matching the display's camera transform; alpha < 1 so the art reads through. */
  private drawGridLines(g: Graphics, pitch: number, color: number, width: number): void {
    const w = this.width;
    const h = this.height;
    const z = this.zoomFactor;
    // Visible world extent (the inverse of screenToWorld at the two corners), snapped out to
    // whole divisions so both edge lines are drawn.
    const wx0 = this.anchorX - w / 2 / z;
    const wy0 = this.anchorY - h / 2 / z;
    const wx1 = this.anchorX + w / 2 / z;
    const wy1 = this.anchorY + h / 2 / z;
    const kx0 = Math.floor(wx0 / pitch);
    const kx1 = Math.ceil(wx1 / pitch);
    const ky0 = Math.floor(wy0 / pitch);
    const ky1 = Math.ceil(wy1 / pitch);
    for (let kx = kx0; kx <= kx1; kx++) {
      const sx = (kx * pitch - this.anchorX) * z + w / 2;
      g.moveTo(sx, 0).lineTo(sx, h);
    }
    for (let ky = ky0; ky <= ky1; ky++) {
      const sy = (ky * pitch - this.anchorY) * z + h / 2;
      g.moveTo(0, sy).lineTo(w, sy);
    }
    g.stroke({ width, color, alpha: 0.5 });
  }

  /** Map a body-local screen point (px, origin at the viewport's top-left) to a WORLD-px
   *  point — the same inverse transform {@link zoomAt} uses. Feed a pointermove through
   *  this into {@link LightRig.setCursorWorld} to attach a hover light. */
  screenToWorld(sx: number, sy: number): { x: number; y: number } {
    const z = this.zoomFactor;
    return { x: this.anchorX + (sx - this.width / 2) / z, y: this.anchorY + (sy - this.height / 2) / z };
  }

  /** Map a WORLD-px point to a body-local screen point (px, origin at the viewport's
   *  top-left) — the inverse of {@link screenToWorld}. Overlays that float above the baked
   *  world (the mover layer's pawn markers) call this every frame to pin a screen-space
   *  sprite to its world position as the camera pans / zooms. */
  worldToScreen(wx: number, wy: number): { x: number; y: number } {
    const z = this.zoomFactor;
    return { x: (wx - this.anchorX) * z + this.width / 2, y: (wy - this.anchorY) * z + this.height / 2 };
  }

  /** Named render-texture channels for the `/showRT` dev preview. Stable order so
   *  the preview tiles don't reshuffle as channels light up: `albedo` is the live
   *  fixed-slot composite (the map itself); `normal`/`emissive` are dormant until
   *  their composites come online (the multi-channel bake), reporting `null`. */
  renderTextures(): RtChannel[] {
    return [
      { name: "albedo-cold", texture: this.map.displayComposite("albedo-cold") },
      { name: "normal-cold", texture: this.map.displayComposite("normal-cold") },
      { name: "surface-cold", texture: this.map.displayComposite("surface-cold") },
      { name: "zdepth-world-cold", texture: this.map.displayComposite("zdepth-world-cold") },
      { name: "lightmap-cold", texture: this.map.displayComposite("lightmap-cold") },
      { name: "albedo-warm", texture: this.warm.displayComposite("albedo-warm") },
      { name: "normal-warm", texture: this.warm.displayComposite("normal-warm") },
      { name: "surface-warm", texture: this.warm.displayComposite("surface-warm") },
      { name: "zdepth-world-warm", texture: this.warm.displayComposite("zdepth-world-warm") },
      { name: "zdepth-hot", texture: this.shadowPass.texture },
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
    // Both caches share the window/slot geometry (identical resize/recenter inputs), so their
    // composites stay slot-aligned for the warm-over-cold composite in the lighting pass.
    this.map.resize(w, h, renderer, z, this.anchorX, this.anchorY);
    this.warm.resize(w, h, renderer, z, this.anchorX, this.anchorY);
    this.map.recenter(this.anchorX, this.anchorY);
    this.warm.recenter(this.anchorX, this.anchorY);
    // Warm has PRIORITY: bake its dirty squares (movers — few) first, then cold gets the
    // remaining budget (floored so a warm flood on a fresh window / LOD swap never fully
    // starves the static world).
    this.warm.bakeDirty(renderer, BAKE_BUDGET);
    // Pack the current cold lights onto the bake shader so a dirty cold rect re-sums them into the
    // cold_lightmap (lighting P1). The bake runs inside `this.map.bakeDirty` (the derived channel).
    this.rig.packCold(this.lightBake);
    this.map.bakeDirty(renderer, Math.max(BAKE_BUDGET - this.warm.lastBaked, COLD_BAKE_FLOOR));
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
    const albedo = this.map.displayComposite("albedo-cold");
    const normal = this.map.displayComposite("normal-cold");
    const surface = this.map.displayComposite("surface-cold");
    const zdepthWorld = this.map.displayComposite("zdepth-world-cold");
    if (albedo && normal && surface && zdepthWorld && this.mesh) {
      this.lightingShader.albedo = albedo;
      this.lightingShader.normal = normal;
      this.lightingShader.surface = surface;
      this.lightingShader.depth = zdepthWorld;
      // Cold lightmap (lighting P1) — the baked static-light sum, added to the display's light sum.
      const coldLightmap = this.map.displayComposite("lightmap-cold");
      if (coldLightmap) this.lightingShader.coldLightmap = coldLightmap;
      // Warm tier: the same four composites for the movers, slot-aligned with cold (same
      // window). The lighting shader composites warm OVER cold per-fragment by warm surface.B,
      // then lights the merged G-buffer — so pawns light + shadow like the world. When warm
      // isn't ready yet its samplers stay EMPTY (coverage 0 → pure cold).
      const albedoW = this.warm.displayComposite("albedo-warm");
      const normalW = this.warm.displayComposite("normal-warm");
      const surfaceW = this.warm.displayComposite("surface-warm");
      const zdepthWorldW = this.warm.displayComposite("zdepth-world-warm");
      if (albedoW && normalW && surfaceW && zdepthWorldW) {
        this.lightingShader.albedoWarm = albedoW;
        this.lightingShader.normalWarm = normalW;
        this.lightingShader.surfaceWarm = surfaceW;
        this.lightingShader.depthWarm = zdepthWorldW;
      }
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

    // Debug tile grid (the `?grid` param): a red gfx overlay redrawn against the live camera.
    this.drawGrid();
  }

  /** The shadow-pass caster list: each standing prim's billboard box + its resolved SURFACE
   *  texture (its B channel is the silhouette/coverage the shadow rasterizes) + a placeholder
   *  depth. Skips prims still on the geo tier (no surface) so only true silhouettes cast. */
  private buildCasters(renderer: Renderer): ShadowCaster[] {
    const out: ShadowCaster[] = [];
    // Cold things AND warm movers cast — both are standing prims with a resolved silhouette.
    for (const p of [...this.map.standingPrims(), ...this.warm.standingPrims()]) {
      if (!p.textureName) continue;
      // Only cast from things on the real tier (albedo AND surface loaded) — same gate as the
      // depth/normal/surface bakes — so a geo-tier thing (still a box in albedo) doesn't throw
      // a shadow from an invisible caster.
      const alb = this.resolver.resolve(p.textureName, "albedo", p.cell);
      const { texture, geo } = this.resolver.resolve(p.textureName, "surface", p.cell);
      if (alb.geo || geo) continue;
      // Sprite bounding box (present pixels) — read back from the GPU once per stem, then cached
      // (fall back to the whole box if the readback fails, so the caster still shadows). A linked
      // cell keys by (stem, cell) since each cell has its own silhouette. The shadow floors derive
      // from it in the shadow pass.
      const okey = p.cell != null ? `${p.textureName}#${p.cell}` : p.textureName;
      let bbox = this.bboxCache.get(okey);
      if (bbox === undefined) {
        bbox = spriteBBox(renderer, texture) ?? FULL_BBOX;
        this.bboxCache.set(okey, bbox);
      }
      out.push({
        x: p.x,
        y: p.y,
        width: p.width,
        height: p.height,
        texture,
        flipX: !!p.flipX,
        depth: Math.max(4, p.width * 0.15), // placeholder until a real per-def depth lands
        // Facing → billboard roll: the texture stem's trailing segment is the facing (`s`/`e`/`n`,
        // west ships as `e`+flipX). A front/back (`s`/`n`) sprite rolls its card edge-on (±1, sign
        // per s vs n); a side (`e`) or linked (`l`) sprite uses the standard billboard (0).
        nsRoll: nsRollFor(p.textureName),
        bbox,
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
      this.mesh.zIndex = 0; // below the overlay (zIndex 1)
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
    this.overlayContainer.destroy({ children: true });
    this.map.destroy();
    this.warm.destroy();
    this.shadowPass.destroy();
    this.mesh?.geometry.destroy();
    this.lightingShader.destroy();
    this.posBuf?.destroy();
    this.uvBuf?.destroy();
    super.destroy();
  }
}
