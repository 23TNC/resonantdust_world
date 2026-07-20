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

  // ── light-data-texture experiment ──────────────────────────────────────────────
  // A 5×5 RGBA32F data texture: column x = light index, rows y = the 5 data pixels (see the field table
  // in docs/work/light-data-texture). Row 3 = the light's RGBA colour, which the DISPLAY shader samples
  // (no hardcoded palette). Re-uploaded every frame with fresh random colours to prove the live path.
  private readonly lightData = new Float32Array(5 * 5 * 4);
  private lightTex: Texture | null = null;
  /** The colour written to each light's row-3 this frame (mirror of the texture, for the debug markers). */
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
    // 5×5 float data texture over `lightData`. RGBA32F → 4× full-precision f32 per texel (ES 1.00 rules
    // out true u32 integer textures; float is the full-precision path — light-data-texture F1). `nearest`
    // so texel-centre samples land exactly on one light/row (I-5).
    this.lightTex = new Texture({
      source: new BufferImageSource({ resource: this.lightData, width: 5, height: 5, format: "rgba32float", scaleMode: "nearest" }),
    });
  }

  /** Pack every light into its column of the data texture and re-upload. When `rollColors`, row 3 gets a
   *  fresh random colour (mirrored into `curColors` for the markers) — the change that proves the shader
   *  is genuinely reading the texture; otherwise the existing `curColors` are re-written unchanged. Colours
   *  are re-rolled once/sec (COLOR_INTERVAL_MS). Float mode stores world-px directly, so region/zone/tile
   *  stay 0. */
  private fillLightData(rollColors: boolean): void {
    const d = this.lightData;
    for (let k = 0; k < this.lights.length; k++) {
      const L = this.lights[k];
      const px = (row: number): number => (row * 5 + k) * 4; // texel (col=k, row) → base index
      // row 0 — region_x, region_y, zone_x, zone_y (unused in float mode)
      d[px(0) + 0] = 0; d[px(0) + 1] = 0; d[px(0) + 2] = 0; d[px(0) + 3] = 0;
      // row 1 — tile_x, tile_y, anchor_x, anchor_y (world px)
      d[px(1) + 0] = 0; d[px(1) + 1] = 0; d[px(1) + 2] = L.x; d[px(1) + 3] = L.y;
      // row 2 — anchor_z, radius, intensity, reserved
      d[px(2) + 0] = L.z; d[px(2) + 1] = L.radius; d[px(2) + 2] = 1; d[px(2) + 3] = 0;
      // row 3 — red, green, blue, alpha (bright-biased random on a re-roll, held otherwise)
      const c = this.curColors[k];
      if (rollColors) {
        c.r = 0.3 + 0.7 * Math.random(); c.g = 0.3 + 0.7 * Math.random(); c.b = 0.3 + 0.7 * Math.random();
      }
      d[px(3) + 0] = c.r; d[px(3) + 1] = c.g; d[px(3) + 2] = c.b; d[px(3) + 3] = 1;
      // row 4 — reserved (growth)
      d[px(4) + 0] = 0; d[px(4) + 1] = 0; d[px(4) + 2] = 0; d[px(4) + 3] = 0;
    }
    this.lightTex!.source.update();
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

  /** Cast the dirty lights' billboard shadows in SCREEN space into `dst`, each light in its own bit. */
  private castScreen(renderer: Renderer, dst: RenderTexture, dirty: number[], prims: Primitive[], cam: Cam): void {
    this.clearRT(renderer, dst); // A=1 opaque, all bits clear
    for (const k of dirty) {
      const L = this.lights[k];
      const g = this.castGfx;
      g.clear();
      const sx = (wx: number): number => (wx + cam.panX) * cam.zoom;
      const sy = (wy: number): number => (wy + cam.panY) * cam.zoom;
      for (const p of prims) {
        const cx = p.x + p.width / 2;
        const baseY = p.y + p.height;
        if (Math.hypot(cx - L.x, baseY - L.y) > L.radius) continue;
        const W = p.width;
        const H = p.height;
        const factor = L.z <= H + 1 ? TMAX : Math.min(L.z / (L.z - H), TMAX);
        const bLx = cx - W / 2;
        const bRx = cx + W / 2;
        const tLx = L.x + factor * (bLx - L.x);
        const tRx = L.x + factor * (bRx - L.x);
        const tY = L.y + factor * (baseY - L.y);
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

    // 1. Cast the dirty lights' shadows in screen space → cur-screen; stash the camera it was cast at.
    curCam.panX = panX; curCam.panY = panY; curCam.zoom = zoom; curCam.vw = vw; curCam.vh = vh;
    this.castScreen(renderer, curScreen, dirty, prims, curCam);

    // 2. Merge: cur-world = (prev-world − dirty) OR prev-screen(remapped with prev-screen's cast cam).
    this.merge.prevWorld = prevWorld;
    this.merge.prevScreen = prevScreen;
    this.merge.setClear(dirtyMask);
    this.merge.setMapping(m.winCol, m.winRow, m.cols, m.rows, m.slotPx, m.fixedCW, m.fixedCH, SQUARE, this.lastWinCol, this.lastWinRow);
    this.merge.setCam(prevCam.panX, prevCam.panY, prevCam.zoom, prevCam.vw, prevCam.vh);
    renderer.render({ container: this.mergeMesh!, target: curWorld, clear: true, clearColor: [0, 0, 0, 1] });
    this.lastWinCol = m.winCol; // this frame's window becomes prev-world's window for next tick's invalidation
    this.lastWinRow = m.winRow;

    // Pack the lights into the data texture + re-upload — the display reads colour straight from here.
    // Re-roll the random colours once/sec (not every frame); lastColorMs starts in the past so frame 1 rolls.
    const rollColors = now - this.lastColorMs >= COLOR_INTERVAL_MS;
    if (rollColors) this.lastColorMs = now;
    this.fillLightData(rollColors);

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
