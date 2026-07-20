//! shadow-world experiment (edit of shadow-cast) — cast 5 cold lights' shadows into a WORLD-space
//! ping-pong bitfield, one bit per light (RED byte, bits 0–4). The bitfield RTs live in the SAME toroidal
//! layout as the composites (`albedo-cold` etc.), so shadows stick to the world under pan/zoom. The
//! coloured display is GONE — inspect via `/overlayRT shadow-a` / `shadow-b` (the overlay decodes the
//! bits to 5 colours). Only the light dot + radius-ring markers draw here (screen-space debug).
//!
//! Per update (one light moves/second): cast the dirty light's in-radius billboard shadows onto the
//! GROUND in world coords → world-space `mask` (mapped into the toroidal buffer, ±period copies for the
//! wrap); combine reads `src` bitfield + `mask` → writes `dest` (clear bit k, set where masked, carry the
//! other 4 bits); swap. Source ≠ destination (no feedback). unorm RGBA8, A=1, float-mod (ES 1.00).

import { Container, Buffer, BufferUsage, Geometry, Graphics, Mesh, RenderTexture, type Renderer } from "pixi.js";
import type { Primitive } from "./SquareCache";
import { SQUARE } from "./squareMath";
import { makeShadowCombineShader, type ShadowCombineShader } from "./shadowCastShaders";

interface Light {
  x: number;
  y: number;
  z: number;
  radius: number;
}

/** The toroidal buffer mapping the shadow RTs share with the composites (from `SquareCache.bufferMapping`). */
interface Mapping {
  fixedCW: number;
  fixedCH: number;
  cols: number;
  rows: number;
  slotPx: number;
}

const ZONE_X0 = 96 * 64;
const ZONE_Y0 = 48 * 64;
const ZONE_SPAN = 16 * 64;
const LIGHT_Z = 480;
const LIGHT_RADIUS = 4 * 64;
const MOVE_INTERVAL_MS = 1000;
const TMAX = 3;
const LIGHT_COLORS = [0xff4040, 0x40ff4d, 0x4d8cff, 0xfff240, 0xff59ff];

const wmod = (n: number, m: number): number => ((n % m) + m) % m;

export class ShadowCast {
  private readonly lights: Light[] = [
    { x: ZONE_X0 + 260, y: ZONE_Y0 + 300, z: LIGHT_Z, radius: LIGHT_RADIUS },
    { x: ZONE_X0 + 720, y: ZONE_Y0 + 260, z: LIGHT_Z, radius: LIGHT_RADIUS },
    { x: ZONE_X0 + 420, y: ZONE_Y0 + 620, z: LIGHT_Z, radius: LIGHT_RADIUS },
    { x: ZONE_X0 + 820, y: ZONE_Y0 + 720, z: LIGHT_Z, radius: LIGHT_RADIUS },
    { x: ZONE_X0 + 560, y: ZONE_Y0 + 900, z: LIGHT_Z, radius: LIGHT_RADIUS },
  ];
  private queue: number[] = [];
  private nextMove = 0;
  private lastMoveMs = 0;

  private a: RenderTexture | null = null;
  private b: RenderTexture | null = null;
  private mask: RenderTexture | null = null;
  /** True when A holds the CURRENT bitfield; flips each processed light. */
  private curIsA = true;
  /** Last buffer mapping — recreate RTs when the fixed size changes, re-seed when the toroidal grid does. */
  private map: Mapping = { fixedCW: 0, fixedCH: 0, cols: 0, rows: 0, slotPx: 0 };
  private running = false;
  private seeded = false;

  private readonly castGfx = new Graphics();
  private readonly markerGfx = new Graphics();
  private readonly combine: ShadowCombineShader = makeShadowCombineShader();
  private geo: Geometry | null = null;
  private combineMesh: Mesh<Geometry> | null = null;
  /** Screen-space markers layer — add to the viewport's overlay. */
  readonly container = new Container();

  constructor() {
    this.container.addChild(this.markerGfx);
    this.container.visible = false;
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

  /** The world-space bitfield RT for `/overlayRT` — `shadow-a` = buffer A, `shadow-b` = buffer B. Both
   *  hold a complete field (carry-forward); the latest is whichever was written most recently. */
  texFor(name: string): RenderTexture | null {
    if (!this.running) return null;
    if (name === "shadow-a") return this.a;
    if (name === "shadow-b") return this.b;
    return null;
  }

  private ensure(renderer: Renderer, m: Mapping): boolean {
    const sizeChanged = m.fixedCW !== this.map.fixedCW || m.fixedCH !== this.map.fixedCH;
    const gridChanged = m.cols !== this.map.cols || m.rows !== this.map.rows || m.slotPx !== this.map.slotPx;
    this.map = m;
    if (m.fixedCW <= 0 || m.fixedCH <= 0) return false;
    if (sizeChanged || !this.a) {
      for (const rt of [this.a, this.b, this.mask]) rt?.destroy(true);
      const mk = (): RenderTexture => {
        const rt = RenderTexture.create({ width: m.fixedCW, height: m.fixedCH, resolution: 1 });
        rt.source.scaleMode = "nearest";
        return rt;
      };
      this.a = mk();
      this.b = mk();
      this.mask = mk();
      this.geo?.destroy(true);
      const pos = new Float32Array([0, 0, m.fixedCW, 0, m.fixedCW, m.fixedCH, 0, m.fixedCH]);
      const uv = new Float32Array([0, 0, 1, 0, 1, 1, 0, 1]);
      this.geo = new Geometry({
        attributes: {
          aPosition: { buffer: new Buffer({ data: pos, usage: BufferUsage.VERTEX }), format: "float32x2" },
          aUV: { buffer: new Buffer({ data: uv, usage: BufferUsage.VERTEX }), format: "float32x2" },
        },
        indexBuffer: new Buffer({ data: new Uint32Array([0, 1, 2, 0, 2, 3]), usage: BufferUsage.INDEX }),
      });
      if (!this.combineMesh) this.combineMesh = new Mesh<Geometry>({ geometry: this.geo, shader: this.combine });
      else this.combineMesh.geometry = this.geo;
    }
    if (sizeChanged || gridChanged) {
      // The toroidal mapping changed (resize / zoom) → the stored bitfields are misaligned. Clear both
      // and re-cast all 5 lights fresh (nearest-reproject would garble a bitfield; refill is simpler).
      const empty = new Container();
      renderer.render({ container: empty, target: this.a!, clear: true, clearColor: [0, 0, 0, 1] });
      renderer.render({ container: empty, target: this.b!, clear: true, clearColor: [0, 0, 0, 1] });
      this.seeded = false;
    }
    return true;
  }

  /** Cast the dirty light `k`'s billboard shadows onto the ground (world), map into the toroidal buffer,
   *  rasterise into `mask` with ±period copies for the wrap. */
  private castLight(renderer: Renderer, k: number, prims: Primitive[]): void {
    const L = this.lights[k];
    const { cols, rows, slotPx } = this.map;
    const per = slotPx / SQUARE; // buffer px per world px
    const g = this.castGfx;
    g.clear();
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
      // Map to the toroidal buffer: reference = the first corner's mod slot, others offset from it (so a
      // seam-crossing quad isn't torn; the ±period copies below cover the wrap). +1 slot = the apron ring.
      const refBx = (wmod(bLx / SQUARE, cols) + 1) * slotPx;
      const refBy = (wmod(baseY / SQUARE, rows) + 1) * slotPx;
      const bx = (wx: number): number => refBx + (wx - bLx) * per;
      const by = (wy: number): number => refBy + (wy - baseY) * per;
      g.poly([bx(bLx), by(baseY), bx(bRx), by(baseY), bx(tRx), by(tY), bx(tLx), by(tY)]).fill(0xffffff);
    }
    // Rasterise into `mask`, re-drawing at ±period offsets so quads that cross the buffer seam wrap.
    const periodX = cols * slotPx;
    const periodY = rows * slotPx;
    let first = true;
    for (const ox of [-periodX, 0, periodX]) {
      for (const oy of [-periodY, 0, periodY]) {
        g.position.set(ox, oy);
        renderer.render({ container: g, target: this.mask!, clear: first, clearColor: [0, 0, 0, 1] });
        first = false;
      }
    }
    g.position.set(0, 0);
  }

  /** Per-frame. `m` = the cache's buffer mapping; `prims` the world casters; `pan`/`z` map world→screen
   *  for the markers only (the shadows are world-space now). */
  tick(renderer: Renderer, m: Mapping, prims: Primitive[], panX: number, panY: number, z: number): void {
    if (!this.running) return;
    if (!this.ensure(renderer, m)) return;
    const now = performance.now();
    if (!this.seeded) {
      this.queue = [0, 1, 2, 3, 4];
      this.seeded = true;
      this.lastMoveMs = now;
      this.nextMove = 0;
      this.curIsA = true;
    }
    if (now - this.lastMoveMs >= MOVE_INTERVAL_MS) {
      this.lastMoveMs = now;
      const k = this.nextMove;
      this.nextMove = (this.nextMove + 1) % this.lights.length;
      this.lights[k].x = ZONE_X0 + Math.random() * ZONE_SPAN;
      this.lights[k].y = ZONE_Y0 + Math.random() * ZONE_SPAN;
      this.queue.push(k);
    }
    if (this.queue.length > 0) {
      const k = this.queue.shift()!;
      const srcRT = this.curIsA ? this.a! : this.b!;
      const dstRT = this.curIsA ? this.b! : this.a!;
      this.castLight(renderer, k, prims);
      this.combine.field = srcRT;
      this.combine.mask = this.mask!;
      this.combine.setBit(k);
      renderer.render({ container: this.combineMesh!, target: dstRT, clear: true, clearColor: [0, 0, 0, 1] });
      this.curIsA = !this.curIsA;
    }
    this.drawMarkers(panX, panY, z);
  }

  /** Each light as a colour-matched dot (thick black outline) + its radius ring (thick black outline),
   *  in screen px — to eyeball which prims fall in a light's radius. */
  private drawMarkers(panX: number, panY: number, z: number): void {
    const g = this.markerGfx;
    g.clear();
    for (let k = 0; k < this.lights.length; k++) {
      const L = this.lights[k];
      const sx = (L.x + panX) * z;
      const sy = (L.y + panY) * z;
      g.circle(sx, sy, L.radius * z).stroke({ width: 4, color: 0x000000, alpha: 1 });
      g.circle(sx, sy, 10).fill({ color: LIGHT_COLORS[k], alpha: 1 }).stroke({ width: 4, color: 0x000000, alpha: 1 });
    }
  }

  destroy(): void {
    for (const rt of [this.a, this.b, this.mask]) rt?.destroy(true);
    this.geo?.destroy(true);
    this.castGfx.destroy();
    this.markerGfx.destroy();
    this.combine.destroy();
    this.container.destroy({ children: true });
  }
}
