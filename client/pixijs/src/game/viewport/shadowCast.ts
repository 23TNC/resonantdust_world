//! shadow-cast experiment — cast shadows from in-radius prims through 5 cold lights into a ping-pong
//! bitfield RT, one bit per light (RED byte, bits 0–4), displayed as 5 colours. Move one light/second and
//! re-cast ONLY the dirty light: cast its shadows → `mask`, then combine `mask` + `src` → `dest` (clear
//! bit k, set it where the mask covers, carry the other 4 bits). Ping-pong `shadow-a`/`shadow-b`.
//!
//! A miniature of the real pipeline: `mask` ≈ shadow-hot (a light's coverage), a/b ≈ shadow-cold (the
//! bitfield). Screen-space; unorm RGBA8, A=1 (alpha never carries data). Toggled via `/shadowcast`.

import { Container, Buffer, BufferUsage, Geometry, Graphics, Mesh, RenderTexture, Texture, type Renderer } from "pixi.js";
import type { Primitive } from "./SquareCache";
import { makeShadowCombineShader, makeShadowDisplayShader, type ShadowCombineShader, type ShadowDisplayShader } from "./shadowCastShaders";

interface Light {
  x: number;
  y: number;
  z: number;
  radius: number;
}

/** The (100,50) zone in world px — origin tile (96,48), 16 tiles = 1024 px. Lights spawn/move here. */
const ZONE_X0 = 96 * 64;
const ZONE_Y0 = 48 * 64;
const ZONE_SPAN = 16 * 64;
const LIGHT_Z = 480; // well above the casters → short, readable shadows (factor ≈ 1.5)
const LIGHT_RADIUS = 4 * 64; // fewer casters per light → the 5 colours stay legible
const MOVE_INTERVAL_MS = 1000;
const TMAX = 3; // clamp runaway grazing shadow projections

export class ShadowCast {
  private readonly lights: Light[] = [
    { x: ZONE_X0 + 260, y: ZONE_Y0 + 300, z: LIGHT_Z, radius: LIGHT_RADIUS },
    { x: ZONE_X0 + 720, y: ZONE_Y0 + 260, z: LIGHT_Z, radius: LIGHT_RADIUS },
    { x: ZONE_X0 + 420, y: ZONE_Y0 + 620, z: LIGHT_Z, radius: LIGHT_RADIUS },
    { x: ZONE_X0 + 820, y: ZONE_Y0 + 720, z: LIGHT_Z, radius: LIGHT_RADIUS },
    { x: ZONE_X0 + 560, y: ZONE_Y0 + 900, z: LIGHT_Z, radius: LIGHT_RADIUS },
  ];
  /** Lights waiting to be (re)cast — a dirty queue; one processed per tick. */
  private queue: number[] = [];
  private nextMove = 0;
  private lastMoveMs = 0;

  private a: RenderTexture | null = null;
  private b: RenderTexture | null = null;
  private aTex: Texture | null = null;
  private bTex: Texture | null = null;
  private mask: RenderTexture | null = null;
  private maskTex: Texture | null = null;
  /** True when A holds the CURRENT bitfield; flips each processed light. */
  private curIsA = true;
  private w = 0;
  private h = 0;
  private running = false;
  private seeded = false;

  private readonly castGfx = new Graphics();
  private readonly combine: ShadowCombineShader = makeShadowCombineShader();
  private readonly display: ShadowDisplayShader = makeShadowDisplayShader();
  private geo: Geometry | null = null;
  private combineMesh: Mesh<Geometry> | null = null;
  private displayMesh: Mesh<Geometry> | null = null;
  /** Screen-space display object — add to the viewport's overlay layer. */
  readonly container = new Container();

  constructor() {
    this.container.visible = false;
  }

  get enabled(): boolean {
    return this.running;
  }
  /** Toggle on/off. On → re-seed (cast all 5 lights fresh). */
  toggle(): boolean {
    this.running = !this.running;
    this.container.visible = this.running;
    if (this.running) this.seeded = false;
    return this.running;
  }

  private ensure(w: number, h: number, renderer: Renderer): void {
    if (this.a && this.w === w && this.h === h) return;
    this.w = w;
    this.h = h;
    for (const rt of [this.a, this.b, this.mask]) rt?.destroy(true);
    for (const t of [this.aTex, this.bTex, this.maskTex]) t?.destroy();
    const mk = (): [RenderTexture, Texture] => {
      const rt = RenderTexture.create({ width: w, height: h, resolution: 1 });
      rt.source.scaleMode = "nearest";
      return [rt, new Texture({ source: rt.source, dynamic: true })];
    };
    [this.a, this.aTex] = mk();
    [this.b, this.bTex] = mk();
    [this.mask, this.maskTex] = mk();
    // Clear both bitfields opaque (A=1, all bits clear).
    const empty = new Container();
    renderer.render({ container: empty, target: this.a, clear: true, clearColor: [0, 0, 0, 1] });
    renderer.render({ container: empty, target: this.b, clear: true, clearColor: [0, 0, 0, 1] });

    this.geo?.destroy(true);
    const pos = new Float32Array([0, 0, w, 0, w, h, 0, h]);
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
    if (!this.displayMesh) {
      this.displayMesh = new Mesh<Geometry>({ geometry: this.geo, shader: this.display });
      this.container.addChild(this.displayMesh);
    } else {
      this.displayMesh.geometry = this.geo;
    }
    this.seeded = false;
  }

  /** Cast the dirty light `k`'s billboard shadows (its in-radius prims) into `mask`, in screen px. */
  private castLight(renderer: Renderer, k: number, prims: Primitive[], panX: number, panY: number, z: number): void {
    const L = this.lights[k];
    const g = this.castGfx;
    g.clear();
    const sx = (wx: number): number => (wx + panX) * z;
    const sy = (wy: number): number => (wy + panY) * z;
    for (const p of prims) {
      const cx = p.x + p.width / 2;
      const baseY = p.y + p.height;
      if (Math.hypot(cx - L.x, baseY - L.y) > L.radius) continue; // in-radius cull
      const W = p.width;
      const H = p.height;
      // Project the billboard's top edge (height H) from the light to the ground; the base edge stays put.
      const factor = L.z <= H + 1 ? TMAX : Math.min(L.z / (L.z - H), TMAX);
      const bLx = cx - W / 2;
      const bRx = cx + W / 2;
      const tLx = L.x + factor * (bLx - L.x);
      const tRx = L.x + factor * (bRx - L.x);
      const tY = L.y + factor * (baseY - L.y);
      // Shadow quad: base-left, base-right, projected-top-right, projected-top-left (screen px).
      g.poly([sx(bLx), sy(baseY), sx(bRx), sy(baseY), sx(tRx), sy(tY), sx(tLx), sy(tY)]).fill(0xffffff);
    }
    renderer.render({ container: g, target: this.mask!, clear: true, clearColor: [0, 0, 0, 1] });
  }

  /** Per-frame: seed on first run; move one light/second; process one dirty light (cast → combine →
   *  swap) if any; display the current bitfield. `prims` are the world casters; `pan`/`z` map world→screen. */
  tick(renderer: Renderer, w: number, h: number, prims: Primitive[], panX: number, panY: number, z: number): void {
    if (!this.running || w <= 0 || h <= 0) return;
    this.ensure(w, h, renderer);
    const now = performance.now();
    if (!this.seeded) {
      this.queue = [0, 1, 2, 3, 4]; // cast all 5 fresh
      this.seeded = true;
      this.lastMoveMs = now;
      this.nextMove = 0;
      this.curIsA = true;
    }
    // Move one light every second → it becomes the dirty light to re-cast.
    if (now - this.lastMoveMs >= MOVE_INTERVAL_MS) {
      this.lastMoveMs = now;
      const k = this.nextMove;
      this.nextMove = (this.nextMove + 1) % this.lights.length;
      this.lights[k].x = ZONE_X0 + Math.random() * ZONE_SPAN;
      this.lights[k].y = ZONE_Y0 + Math.random() * ZONE_SPAN;
      this.queue.push(k);
    }
    // Process ONE dirty light: cast → mask, combine src+mask → dest (rewrite bit k), swap.
    if (this.queue.length > 0) {
      const k = this.queue.shift()!;
      const srcTex = this.curIsA ? this.aTex! : this.bTex!;
      const dstRT = this.curIsA ? this.b! : this.a!;
      this.castLight(renderer, k, prims, panX, panY, z);
      this.combine.field = srcTex;
      this.combine.mask = this.maskTex!;
      this.combine.setBit(k);
      renderer.render({ container: this.combineMesh!, target: dstRT, clear: true, clearColor: [0, 0, 0, 1] });
      this.curIsA = !this.curIsA; // dest is now current
    }
    this.display.field = this.curIsA ? this.aTex! : this.bTex!;
  }

  destroy(): void {
    for (const rt of [this.a, this.b, this.mask]) rt?.destroy(true);
    for (const t of [this.aTex, this.bTex, this.maskTex]) t?.destroy();
    this.geo?.destroy(true);
    this.castGfx.destroy();
    this.combine.destroy();
    this.display.destroy();
    this.container.destroy({ children: true });
  }
}
