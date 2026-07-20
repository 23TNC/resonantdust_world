//! bitfield-rt E5 — the ping-pong read-modify-write proof. Two bitfield RTs (A, B); each frame the STEP
//! shader reads the CURRENT one and writes the OTHER (marching every cell's set bit +1 mod 24), then the
//! newly-written RT is DISPLAYED the same frame via `overlayShader`'s BITS decode. Next frame the roles
//! swap. Proves: read a bitfield from a texture → write an updated bitfield to another → display it same
//! frame → alternate, with state persisting across the ping-pong.
//!
//! Source ≠ destination every step (no feedback loop). Screen-space (this proves the RMW/ping-pong/
//! same-frame-display mechanic; E1–E4 already proved world-space storage). Toggled via `/bitpingpong`.

import { Buffer, BufferUsage, Container, Geometry, Mesh, RenderTexture, Texture, type Renderer } from "pixi.js";
import { makeBitStepShader, type BitStepShader } from "./bitStepShader";
import { makeOverlayShader, OVERLAY_BITS, type OverlayShader } from "./overlayShader";

export class BitPingPong {
  private a: RenderTexture | null = null;
  private b: RenderTexture | null = null;
  private aTex: Texture | null = null;
  private bTex: Texture | null = null;
  /** True when A is the SOURCE this frame (B the destination); flips every step. */
  private srcIsA = true;
  /** False until the first (seed) step has run — the seed step ignores the source. */
  private seeded = false;
  private w = 0;
  private h = 0;
  private res = 1;
  private running = false;

  private readonly step: BitStepShader = makeBitStepShader();
  private geo: Geometry | null = null;
  private stepMesh: Mesh<Geometry> | null = null;
  private readonly displayShader: OverlayShader = makeOverlayShader();
  private displayMesh: Mesh<Geometry> | null = null;
  /** The screen-space display object — add to the viewport's overlay layer. */
  readonly container = new Container();

  constructor() {
    this.displayShader.mode = OVERLAY_BITS;
    this.container.visible = false;
  }

  get enabled(): boolean {
    return this.running;
  }
  /** Toggle the proof on/off. Turning on re-seeds on the next tick. */
  toggle(): boolean {
    this.running = !this.running;
    this.container.visible = this.running;
    if (this.running) this.seeded = false;
    return this.running;
  }

  private ensure(w: number, h: number, res: number, renderer: Renderer): void {
    if (this.a && this.w === w && this.h === h && this.res === res) return;
    this.w = w;
    this.h = h;
    this.res = res;
    this.a?.destroy(true);
    this.b?.destroy(true);
    this.aTex?.destroy();
    this.bTex?.destroy();
    this.a = RenderTexture.create({ width: w, height: h, resolution: res });
    this.b = RenderTexture.create({ width: w, height: h, resolution: res });
    this.a.source.scaleMode = "nearest";
    this.b.source.scaleMode = "nearest";
    this.aTex = new Texture({ source: this.a.source, dynamic: true });
    this.bTex = new Texture({ source: this.b.source, dynamic: true });
    // Clear both so a stray read before seeding is defined (all bits clear).
    renderer.render({ container: new Container(), target: this.a, clear: true, clearColor: [0, 0, 0, 1] });
    renderer.render({ container: new Container(), target: this.b, clear: true, clearColor: [0, 0, 0, 1] });

    // One full-buffer quad (0..w, 0..h; uv 0..1), shared by the step mesh (rendered into an RT) and the
    // display mesh (a scene child). Rebuilt on resize.
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
    if (!this.stepMesh) this.stepMesh = new Mesh<Geometry>({ geometry: this.geo, shader: this.step });
    else this.stepMesh.geometry = this.geo;
    if (!this.displayMesh) {
      this.displayMesh = new Mesh<Geometry>({ geometry: this.geo, shader: this.displayShader });
      this.container.addChild(this.displayMesh);
    } else {
      this.displayMesh.geometry = this.geo;
    }
    this.seeded = false;
  }

  /** Per-frame: read the current buffer → write the other (marching bits) → display the just-written
   *  buffer this frame → swap. No-op when disabled. */
  tick(renderer: Renderer, w: number, h: number, res: number): void {
    if (!this.running || w <= 0 || h <= 0) return;
    this.ensure(w, h, res, renderer);
    const srcTex = this.srcIsA ? this.aTex! : this.bTex!;
    const dstRT = this.srcIsA ? this.b! : this.a!;
    const dstTex = this.srcIsA ? this.bTex! : this.aTex!;
    // READ src → WRITE dst (source ≠ destination: no feedback loop).
    this.step.field = srcTex;
    this.step.setSeed(this.seeded ? 0 : 1);
    renderer.render({ container: this.stepMesh!, target: dstRT, clear: true, clearColor: [0, 0, 0, 1] });
    this.seeded = true;
    this.srcIsA = !this.srcIsA; // next frame, the buffer we just wrote is the source
    // DISPLAY the buffer we just wrote, THIS frame (it's unbound now — no feedback).
    this.displayShader.texture = dstTex;
  }

  destroy(): void {
    this.a?.destroy(true);
    this.b?.destroy(true);
    this.aTex?.destroy();
    this.bTex?.destroy();
    this.geo?.destroy(true);
    this.step.destroy();
    this.displayShader.destroy();
    this.container.destroy({ children: true });
  }
}
