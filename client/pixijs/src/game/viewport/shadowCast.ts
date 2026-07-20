//! shadow-tiered experiment (4-RT) — cast the realtime (dirty) lights' shadows in SCREEN space (window-
//! bounded by construction, no aliasing), keep the settled shadows in a WORLD-space toroidal bitfield.
//! One bit per light (RED byte, bits 0–4); A never data; float-mod (ES 1.00).
//!
//! Four RTs: screen-a/-b (screen space, this frame's realtime set) + world-a/-b (toroidal world space,
//! persistent). Ping-pong: on an "a" frame read the -b buffers, write the -a buffers.
//!   1. cast the dirty set's shadows in screen space → cur-screen.
//!   2. cur-world = (prev-world with the dirty bits cleared) OR (prev-screen remapped screen→world)
//!      — one MERGE pass (reads prev-world + prev-screen, writes cur-world; no feedback, no blend).
//!   3. display cur-world (world-aligned) OR cur-screen (screen) → 5 colours.
//! The merge uses prev-screen's CAST-frame camera (stashed) so a pan between cast and merge doesn't slide
//! the shadows (shadow-tiered I-8). Casters = in-radius standing prims; billboard-quad shadows.

import { BufferImageSource, Container, Buffer, BufferUsage, Filter, Geometry, Graphics, Mesh, RenderTexture, Texture, type Renderer } from "pixi.js";
import type { Primitive } from "./SquareCache";
import { SQUARE } from "./squareMath";
import { makeShadowDecodeFilter, makeShadowMergeShader, makeShadowTDisplayShader, type ShadowMergeShader, type ShadowTDisplayShader } from "./shadowCastShaders";

interface Light {
  x: number;
  y: number;
  z: number;
  radius: number;
}
interface Mapping {
  fixedCW: number;
  fixedCH: number;
  cols: number;
  rows: number;
  slotPx: number;
  winCol: number;
  winRow: number;
}
/** Camera at the moment a screen buffer was cast — used to remap it screen→world at merge (I-8). */
interface Cam {
  panX: number;
  panY: number;
  zoom: number;
  vw: number;
  vh: number;
}

const ZONE_X0 = 96 * 64;
const ZONE_Y0 = 48 * 64;
const ZONE_SPAN = 16 * 64;
const LIGHT_Z = 480;
const LIGHT_RADIUS = 4 * 64;
const MOVE_INTERVAL_MS = 1000;
const COLOR_INTERVAL_MS = 1000; // re-roll the lights' colours once/sec
const TMAX = 3;
/** Fill colour that writes light k's bit into the RED byte (2^k << 16). */
const bitColor = (k: number): number => (1 << k) << 16;

// ── caster-lut: standard 1024×12 RGBA32F data texture ──────────────────────────────
// 1024 = fixed cross-platform-safe width; 12 rows = 4 bands of 3, and a definition (light or caster) is
// 3 px = 12 f32, so 4 bands × 1024 = MAX_DEFS slots. The LUT reuses the same shape as a flat index array.
const STD_W = 1024;
const STD_H = 12;
const DEF_PX = 3; // px per definition
const TEX_FLOATS = STD_W * STD_H * 4; // 49152
const MAX_DEFS = STD_W * (STD_H / DEF_PX); // 4096 def slots per def-texture
const LUT_ENTRIES = STD_W * STD_H * 4; // 49152 caster-index entries (4/px)
/** Float base-index of px `p` (0..2) of definition `k` in a STD_W×STD_H RGBA32F array (caster-lut I-5). */
function defBase(k: number, p: number): number {
  const col = k % STD_W;
  const band = (k / STD_W) | 0;
  return ((band * DEF_PX + p) * STD_W + col) * 4;
}
// LUT entry `i` lives at pixel `i>>2`, channel `i&3` → flat float index `(i>>2)*4 + (i&3) = i`, so the LUT
// Float32Array is indexed by entry directly: `lutData[i]`.

export class ShadowCast {
  private readonly lights: Light[] = [
    { x: ZONE_X0 + 260, y: ZONE_Y0 + 300, z: LIGHT_Z, radius: LIGHT_RADIUS },
    { x: ZONE_X0 + 720, y: ZONE_Y0 + 260, z: LIGHT_Z, radius: LIGHT_RADIUS },
    { x: ZONE_X0 + 420, y: ZONE_Y0 + 620, z: LIGHT_Z, radius: LIGHT_RADIUS },
    { x: ZONE_X0 + 820, y: ZONE_Y0 + 720, z: LIGHT_Z, radius: LIGHT_RADIUS },
    { x: ZONE_X0 + 560, y: ZONE_Y0 + 900, z: LIGHT_Z, radius: LIGHT_RADIUS },
  ];
  private nextMove = 0;
  private lastMoveMs = 0;
  private lastColorMs = -1e9; // < -COLOR_INTERVAL_MS so the first frame always rolls
  private seeded = false;

  private worldA: RenderTexture | null = null;
  private worldB: RenderTexture | null = null;
  private screenA: RenderTexture | null = null;
  private screenB: RenderTexture | null = null;
  private camA: Cam = { panX: 0, panY: 0, zoom: 1, vw: 1, vh: 1 };
  private camB: Cam = { panX: 0, panY: 0, zoom: 1, vw: 1, vh: 1 };
  /** True when the "-a" buffers are the ones written this frame. */
  private curIsA = true;
  /** Window (winCol/winRow) of the tick that wrote prev-world — for leading-edge slot invalidation. */
  private lastWinCol = 0;
  private lastWinRow = 0;
  private map: Mapping = { fixedCW: 0, fixedCH: 0, cols: 0, rows: 0, slotPx: 0, winCol: 0, winRow: 0 };
  private vw = 0;
  private vh = 0;
  private running = false;

  // ── caster-lut: casters + per-light range lists in textures (all 1024×12 RGBA32F) ──────────────────
  // LIGHT def (3 px): px0 world_x,_y,_z,radius · px1 rgba colour · px2 start_index,count,—,—.
  // CASTER def (3 px): px0 world_x,_y,_z,facing · px1 width,height,depth1,depth2 · px2 frame_x,_y,_w,_h.
  // LUT: flat caster-index array (4/px). Light k's in-range casters are LUT[start .. start+count).
  // The DISPLAY shader samples the light colour (px1); the cast walks light→LUT→caster off the CPU mirrors
  // (identical to the uploaded textures) — the GPU projection is the next step (caster-lut C5).
  private readonly lightData = new Float32Array(TEX_FLOATS);
  private readonly primData = new Float32Array(TEX_FLOATS);
  private readonly lutData = new Float32Array(LUT_ENTRIES);
  private lightTex: Texture | null = null;
  private primTex: Texture | null = null;
  private lutTex: Texture | null = null;
  /** Standing-prim count at the last caster-texture rebuild — rebuild when it changes (zones stream in). */
  private lastCasterCount = -1;
  /** The colour written to each light's px1 this frame (mirror of the texture, for the debug markers). */
  private readonly curColors: { r: number; g: number; b: number }[] = this.lights.map(() => ({ r: 1, g: 1, b: 1 }));

  private readonly castGfx = new Graphics();
  private readonly markerGfx = new Graphics();
  private readonly merge: ShadowMergeShader = makeShadowMergeShader();
  private readonly display: ShadowTDisplayShader = makeShadowTDisplayShader();
  private mergeGeo: Geometry | null = null; // full world-buffer quad
  private mergeMesh: Mesh<Geometry> | null = null;
  private dispGeo: Geometry | null = null; // full-viewport quad
  private displayMesh: Mesh<Geometry> | null = null;
  readonly container = new Container();

  constructor() {
    this.container.addChild(this.markerGfx);
    this.container.visible = false;
    // Three standard 1024×12 RGBA32F textures. Float (not true u32) — ES 1.00 has no integer textures, and
    // f32 is exact for the integer indices we store (< 2²⁴; caster-lut I-1). `nearest` = texel-centre samples.
    this.lightTex = mkDataTex(this.lightData);
    this.primTex = mkDataTex(this.primData);
    this.lutTex = mkDataTex(this.lutData);
  }

  /** Write each light's px0 (pos + radius) and px1 (colour) into `lightData` (px2 start/count is written by
   *  {@link buildCasterData}). When `rollColors`, px1 gets a fresh random colour (mirrored into `curColors`
   *  for the markers) — re-rolled once/sec (COLOR_INTERVAL_MS), held otherwise. No upload — the caller
   *  ({@link tick}) gates the `lightTex` upload on what actually changed. */
  private fillLightData(rollColors: boolean): void {
    const d = this.lightData;
    for (let k = 0; k < this.lights.length; k++) {
      const L = this.lights[k];
      let b = defBase(k, 0); // px0 — world_x, world_y, world_z, radius
      d[b + 0] = L.x; d[b + 1] = L.y; d[b + 2] = L.z; d[b + 3] = L.radius;
      b = defBase(k, 1); // px1 — red, green, blue, alpha (intensity folded in)
      const c = this.curColors[k];
      if (rollColors) {
        c.r = 0.3 + 0.7 * Math.random(); c.g = 0.3 + 0.7 * Math.random(); c.b = 0.3 + 0.7 * Math.random();
      }
      d[b + 0] = c.r; d[b + 1] = c.g; d[b + 2] = c.b; d[b + 3] = 1;
    }
  }

  /** Rebuild the caster textures from the resident standing prims: write each caster's rect into the prim
   *  texture, then per light cull the in-range casters (`hypot ≤ radius`) into a contiguous LUT run and
   *  record `(start, count)` into the light's px2. Writes the CPU mirrors (`primData`/`lutData`/`lightData`
   *  px2); {@link tick} uploads. This is the light → LUT → caster indirection the design specifies. */
  private buildCasterData(prims: Primitive[]): void {
    const pd = this.primData, lut = this.lutData, ld = this.lightData;
    const n = Math.min(prims.length, MAX_DEFS);
    if (prims.length > MAX_DEFS) console.warn(`[caster-lut] ${prims.length} casters > ${MAX_DEFS} slots — truncated`);
    // 1. Prim texture: each caster's def (world rect; depth/facing/frame reserved for the fuller wedge).
    for (let i = 0; i < n; i++) {
      const p = prims[i];
      let b = defBase(i, 0); pd[b + 0] = p.x; pd[b + 1] = p.y; pd[b + 2] = 0; pd[b + 3] = 0; // world_x/_y/_z, facing
      b = defBase(i, 1); pd[b + 0] = p.width; pd[b + 1] = p.height; pd[b + 2] = 0; pd[b + 3] = 0; // w, h, depth1, depth2
      b = defBase(i, 2); pd[b + 0] = 0; pd[b + 1] = 0; pd[b + 2] = 0; pd[b + 3] = 0; // frame_x/_y/_w/_h
    }
    // 2. LUT + light (start, count): one contiguous run of in-range caster indices per light.
    let cursor = 0;
    for (let k = 0; k < this.lights.length; k++) {
      const lb = defBase(k, 0);
      const Lx = ld[lb + 0], Ly = ld[lb + 1], R = ld[lb + 3];
      const start = cursor;
      let count = 0;
      for (let i = 0; i < n; i++) {
        const cx = pd[defBase(i, 0) + 0] + pd[defBase(i, 1) + 0] / 2;
        const baseY = pd[defBase(i, 0) + 1] + pd[defBase(i, 1) + 1];
        if (Math.hypot(cx - Lx, baseY - Ly) > R) continue;
        if (cursor >= LUT_ENTRIES) { console.warn("[caster-lut] LUT overflow — run truncated"); break; }
        lut[cursor++] = i;
        count++;
      }
      const pb = defBase(k, 2); // px2 — start_index, count, reserved, reserved
      ld[pb + 0] = start; ld[pb + 1] = count; ld[pb + 2] = 0; ld[pb + 3] = 0;
    }
  }

  get enabled(): boolean {
    return this.running;
  }
  toggle(): boolean {
    this.running = !this.running;
    this.container.visible = this.running;
    if (this.running) this.seeded = false;
    return this.running;
  }

  /** `/overlayRT` inspection — the CURRENT world buffer (latest). */
  texFor(name: string): RenderTexture | null {
    if (!this.running) return null;
    if (name === "shadow-a") return this.curIsA ? this.worldA : this.worldB;
    if (name === "shadow-b") return this.curIsA ? this.worldB : this.worldA;
    return null;
  }

  /** The `/showRT` decode filter (lazily built) — colourises a `shadow-*` bitfield thumbnail into
   *  the 5 per-light colours instead of the raw near-black red byte. Stateless, so both thumbnails
   *  share this one instance. */
  get decodeFilter(): Filter {
    if (!this.decodeFilterInst) this.decodeFilterInst = makeShadowDecodeFilter();
    return this.decodeFilterInst;
  }
  private decodeFilterInst: Filter | null = null;

  private mkRT(w: number, h: number): RenderTexture {
    const rt = RenderTexture.create({ width: w, height: h, resolution: 1 });
    rt.source.scaleMode = "nearest";
    return rt;
  }
  private clearRT(renderer: Renderer, rt: RenderTexture): void {
    renderer.render({ container: new Container(), target: rt, clear: true, clearColor: [0, 0, 0, 1] });
  }

  private ensure(renderer: Renderer, m: Mapping, vw: number, vh: number): boolean {
    if (m.fixedCW <= 0 || vw <= 0) return false;
    const worldResized = m.fixedCW !== this.map.fixedCW || m.fixedCH !== this.map.fixedCH;
    const gridChanged = m.cols !== this.map.cols || m.rows !== this.map.rows || m.slotPx !== this.map.slotPx;
    const screenResized = vw !== this.vw || vh !== this.vh;
    this.map = m;
    this.vw = vw;
    this.vh = vh;
    if (worldResized || !this.worldA) {
      this.worldA?.destroy(true);
      this.worldB?.destroy(true);
      this.worldA = this.mkRT(m.fixedCW, m.fixedCH);
      this.worldB = this.mkRT(m.fixedCW, m.fixedCH);
      this.mergeGeo?.destroy(true);
      this.mergeGeo = quadGeo(m.fixedCW, m.fixedCH);
      if (!this.mergeMesh) this.mergeMesh = new Mesh<Geometry>({ geometry: this.mergeGeo, shader: this.merge });
      else this.mergeMesh.geometry = this.mergeGeo;
    }
    if (screenResized || !this.screenA) {
      this.screenA?.destroy(true);
      this.screenB?.destroy(true);
      this.screenA = this.mkRT(vw, vh);
      this.screenB = this.mkRT(vw, vh);
      this.dispGeo?.destroy(true);
      this.dispGeo = quadGeo(vw, vh);
      if (!this.displayMesh) {
        this.displayMesh = new Mesh<Geometry>({ geometry: this.dispGeo, shader: this.display });
        this.container.addChildAt(this.displayMesh, 0); // below the markers
      } else {
        this.displayMesh.geometry = this.dispGeo;
      }
    }
    if (worldResized || gridChanged || screenResized) {
      this.clearRT(renderer, this.worldA!);
      this.clearRT(renderer, this.worldB!);
      this.clearRT(renderer, this.screenA!);
      this.clearRT(renderer, this.screenB!);
      this.seeded = false;
    }
    return true;
  }

  /** Cast the dirty lights' billboard shadows in SCREEN space into `dst`, each light in its own bit — driven
   *  by the light → LUT → caster indirection: read the light's `(start,count)`, walk its LUT run, fetch each
   *  caster's rect from the prim mirror, project the wedge. (Reads the CPU mirrors, byte-identical to the
   *  uploaded textures; the GPU-side read of these textures is caster-lut C5.) */
  private castScreen(renderer: Renderer, dst: RenderTexture, dirty: number[], cam: Cam): void {
    this.clearRT(renderer, dst); // A=1 opaque, all bits clear
    const ld = this.lightData, pd = this.primData, lut = this.lutData;
    for (const k of dirty) {
      const lb = defBase(k, 0);
      const Lx = ld[lb + 0], Ly = ld[lb + 1], Lz = ld[lb + 2];
      const pb = defBase(k, 2);
      const start = ld[pb + 0] | 0, count = ld[pb + 1] | 0;
      const g = this.castGfx;
      g.clear();
      const sx = (wx: number): number => (wx + cam.panX) * cam.zoom;
      const sy = (wy: number): number => (wy + cam.panY) * cam.zoom;
      for (let j = 0; j < count; j++) {
        const c = lut[start + j] | 0; // caster index
        const b0 = defBase(c, 0), b1 = defBase(c, 1);
        const X = pd[b0 + 0], Y = pd[b0 + 1], W = pd[b1 + 0], H = pd[b1 + 1];
        const cx = X + W / 2;
        const baseY = Y + H;
        const factor = Lz <= H + 1 ? TMAX : Math.min(Lz / (Lz - H), TMAX);
        const bLx = cx - W / 2;
        const bRx = cx + W / 2;
        const tLx = Lx + factor * (bLx - Lx);
        const tRx = Lx + factor * (bRx - Lx);
        const tY = Ly + factor * (baseY - Ly);
        g.poly([sx(bLx), sy(baseY), sx(bRx), sy(baseY), sx(tRx), sy(tY), sx(tLx), sy(tY)]).fill(bitColor(k));
      }
      g.blendMode = "add"; // different lights → distinct bits → OR; same light's polys are one fill (union)
      renderer.render({ container: g, target: dst, clear: false });
    }
  }

  /** Per-frame. `m` = cache buffer mapping; `prims` world casters; pan/zoom/viewport for screen. */
  tick(renderer: Renderer, m: Mapping, prims: Primitive[], panX: number, panY: number, zoom: number, vw: number, vh: number): void {
    if (!this.running) return;
    if (!this.ensure(renderer, m, vw, vh)) return;
    const now = performance.now();
    let dirty: number[];
    if (!this.seeded) {
      dirty = [0, 1, 2, 3, 4]; // seed: cast every light
      this.seeded = true;
      this.lastMoveMs = now;
      this.nextMove = 0;
      this.curIsA = true;
      this.lastWinCol = m.winCol; // prev-world is empty on seed — no eviction to invalidate
      this.lastWinRow = m.winRow;
    } else {
      dirty = [];
      if (now - this.lastMoveMs >= MOVE_INTERVAL_MS) {
        this.lastMoveMs = now;
        const k = this.nextMove;
        this.nextMove = (this.nextMove + 1) % this.lights.length;
        this.lights[k].x = ZONE_X0 + Math.random() * ZONE_SPAN;
        this.lights[k].y = ZONE_Y0 + Math.random() * ZONE_SPAN;
        dirty = [k];
      }
    }
    let dirtyMask = 0;
    for (const k of dirty) dirtyMask |= 1 << k;

    const curWorld = this.curIsA ? this.worldA! : this.worldB!;
    const prevWorld = this.curIsA ? this.worldB! : this.worldA!;
    const curScreen = this.curIsA ? this.screenA! : this.screenB!;
    const prevScreen = this.curIsA ? this.screenB! : this.screenA!;
    const curCam = this.curIsA ? this.camA : this.camB;
    const prevCam = this.curIsA ? this.camB : this.camA;

    // Rebuild the light + caster textures. fillLightData writes light pos/colour (colour re-rolls once/sec);
    // buildCasterData writes the prim texture + LUT + light (start,count) when a light moved (dirty) or the
    // resident caster set changed. Upload only what changed. This runs BEFORE the cast, which reads it.
    const rollColors = now - this.lastColorMs >= COLOR_INTERVAL_MS;
    if (rollColors) this.lastColorMs = now;
    this.fillLightData(rollColors);
    const casterCountChanged = prims.length !== this.lastCasterCount;
    const rebuilt = dirty.length > 0 || casterCountChanged;
    if (rebuilt) { this.buildCasterData(prims); this.lastCasterCount = prims.length; }
    if (rebuilt) { this.primTex!.source.update(); this.lutTex!.source.update(); }
    if (rebuilt || rollColors) this.lightTex!.source.update();

    // 1. Cast the dirty lights' shadows in screen space → cur-screen; stash the camera it was cast at.
    curCam.panX = panX; curCam.panY = panY; curCam.zoom = zoom; curCam.vw = vw; curCam.vh = vh;
    this.castScreen(renderer, curScreen, dirty, curCam);

    // 2. Merge: cur-world = (prev-world − dirty) OR prev-screen(remapped with prev-screen's cast cam).
    this.merge.prevWorld = prevWorld;
    this.merge.prevScreen = prevScreen;
    this.merge.setClear(dirtyMask);
    this.merge.setMapping(m.winCol, m.winRow, m.cols, m.rows, m.slotPx, m.fixedCW, m.fixedCH, SQUARE, this.lastWinCol, this.lastWinRow);
    this.merge.setCam(prevCam.panX, prevCam.panY, prevCam.zoom, prevCam.vw, prevCam.vh);
    renderer.render({ container: this.mergeMesh!, target: curWorld, clear: true, clearColor: [0, 0, 0, 1] });
    this.lastWinCol = m.winCol; // this frame's window becomes prev-world's window for next tick's invalidation
    this.lastWinRow = m.winRow;

    // 3. Display cur-world OR cur-screen (this frame's camera for both); colour from the data texture.
    this.display.curWorld = curWorld;
    this.display.curScreen = curScreen;
    this.display.lightData = this.lightTex!;
    this.display.setMapping(m.cols, m.rows, m.slotPx, m.fixedCW, m.fixedCH, SQUARE);
    this.display.setCam(panX, panY, zoom, vw, vh);

    this.drawMarkers(panX, panY, zoom);
    this.curIsA = !this.curIsA;
  }

  private drawMarkers(panX: number, panY: number, z: number): void {
    const g = this.markerGfx;
    g.clear();
    for (let k = 0; k < this.lights.length; k++) {
      const L = this.lights[k];
      const sx = (L.x + panX) * z;
      const sy = (L.y + panY) * z;
      const c = this.curColors[k]; // same colour the shader reads from the texture this frame
      const rgb = (Math.round(c.r * 255) << 16) | (Math.round(c.g * 255) << 8) | Math.round(c.b * 255);
      g.circle(sx, sy, L.radius * z).stroke({ width: 4, color: 0x000000, alpha: 1 });
      g.circle(sx, sy, 10).fill({ color: rgb, alpha: 1 }).stroke({ width: 4, color: 0x000000, alpha: 1 });
    }
  }

  destroy(): void {
    for (const rt of [this.worldA, this.worldB, this.screenA, this.screenB]) rt?.destroy(true);
    this.lightTex?.destroy(true);
    this.primTex?.destroy(true);
    this.lutTex?.destroy(true);
    this.mergeGeo?.destroy(true);
    this.dispGeo?.destroy(true);
    this.castGfx.destroy();
    this.markerGfx.destroy();
    this.merge.destroy();
    this.display.destroy();
    this.decodeFilterInst?.destroy();
    this.container.destroy({ children: true });
  }
}

/** A standard 1024×12 RGBA32F data texture over `data` (nearest). Shared shape for the light, caster and
 *  LUT textures (caster-lut). */
function mkDataTex(data: Float32Array): Texture {
  return new Texture({ source: new BufferImageSource({ resource: data, width: STD_W, height: STD_H, format: "rgba32float", scaleMode: "nearest" }) });
}

/** A `w × h` quad with uv 0..1. */
function quadGeo(w: number, h: number): Geometry {
  return new Geometry({
    attributes: {
      aPosition: { buffer: new Buffer({ data: new Float32Array([0, 0, w, 0, w, h, 0, h]), usage: BufferUsage.VERTEX }), format: "float32x2" },
      aUV: { buffer: new Buffer({ data: new Float32Array([0, 0, 1, 0, 1, 1, 0, 1]), usage: BufferUsage.VERTEX }), format: "float32x2" },
    },
    indexBuffer: new Buffer({ data: new Uint32Array([0, 1, 2, 0, 2, 3]), usage: BufferUsage.INDEX }),
  });
}
