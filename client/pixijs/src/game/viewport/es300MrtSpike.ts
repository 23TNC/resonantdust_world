//! mrt-bakes B1 spike — prove multiple render targets work here before touching the real bake.
//!
//! Finding (the reason for this spike): Pixi v8 attaches all of a `RenderTarget`'s `colorTextures` to the
//! framebuffer (COLOR_ATTACHMENT0+i), but it NEVER calls `gl.drawBuffers([...])` — and WebGL2 writes only to
//! attachment 0 by default. So a fragment with `layout(location=1..3) out` writes nowhere for 1..3 unless we
//! call `gl.drawBuffers` ourselves on the bound FBO (it's per-FBO state, so it persists once set).
//!
//! This renders one ES 3.00 mesh whose fragment writes four DISTINCT colours to four outputs, into a
//! 4-attachment target, then shows the four attachments as a 2×2 grid. Four different colours ⇒ MRT works.

import { Buffer, BufferUsage, Container, Geometry, GlProgram, Mesh, RenderTarget, Shader, Sprite, Texture, type Renderer } from "pixi.js";

const VERT = /* glsl */ `#version 300 es
in vec2 aPosition;
void main() { gl_Position = vec4(aPosition, 0.0, 1.0); }
`;

const FRAG = /* glsl */ `#version 300 es
precision highp float;
layout(location = 0) out vec4 o0;
layout(location = 1) out vec4 o1;
layout(location = 2) out vec4 o2;
layout(location = 3) out vec4 o3;
void main() {
  o0 = vec4(1.0, 0.20, 0.20, 1.0); // red    → attachment 0
  o1 = vec4(0.25, 1.0, 0.30, 1.0); // green  → attachment 1
  o2 = vec4(0.30, 0.55, 1.0, 1.0); // blue   → attachment 2
  o3 = vec4(1.0, 0.95, 0.25, 1.0); // yellow → attachment 3
}
`;

/** Mesh pipe wants a TextureShader — this program samples nothing, so a no-op stub. */
class MrtShader extends Shader {
  get texture(): Texture {
    return Texture.EMPTY;
  }
  set texture(_v: Texture) {
    /* no-op */
  }
}

export class Es300MrtSpike {
  readonly container = new Container();
  private target: RenderTarget | null = null;
  private mesh: Mesh<Geometry> | null = null;
  private running = false;

  constructor() {
    this.container.visible = false;
  }

  toggle(): boolean {
    this.running = !this.running;
    this.container.visible = this.running;
    return this.running;
  }

  /** Render the 4-out mesh into the 4-attachment target (called per frame from the viewport render loop). */
  render(renderer: Renderer): void {
    if (!this.running) return;
    if (!this.target) this.build();
    const gl = (renderer as unknown as { gl: WebGL2RenderingContext }).gl;
    // Ensure the target's FBO exists + is bound, THEN enable all four draw buffers on it (Pixi doesn't).
    // drawBuffers is per-FBO state, so this sticks for the subsequent render into the same FBO.
    renderer.renderTarget.bind(this.target!, false);
    gl.drawBuffers([gl.COLOR_ATTACHMENT0, gl.COLOR_ATTACHMENT1, gl.COLOR_ATTACHMENT2, gl.COLOR_ATTACHMENT3]);
    renderer.render({ container: this.mesh!, target: this.target!, clear: true, clearColor: [0, 0, 0, 1] });
  }

  private build(): void {
    const S = 128;
    this.target = new RenderTarget({ width: S, height: S, colorTextures: 4, resolution: 1 });
    const geo = new Geometry({
      attributes: {
        aPosition: { buffer: new Buffer({ data: new Float32Array([-1, -1, 1, -1, 1, 1, -1, 1]), usage: BufferUsage.VERTEX }), format: "float32x2" },
      },
      indexBuffer: new Buffer({ data: new Uint32Array([0, 1, 2, 0, 2, 3]), usage: BufferUsage.INDEX }),
    });
    const program = GlProgram.from({ vertex: VERT, fragment: FRAG, name: "es300-mrt-spike" });
    this.mesh = new Mesh<Geometry>({ geometry: geo, shader: new MrtShader({ glProgram: program, resources: {} }) });
    // Show the four attachments as a 2×2 grid, top-left of the screen.
    for (let i = 0; i < 4; i++) {
      const spr = new Sprite(new Texture({ source: this.target.colorTextures[i] }));
      spr.width = spr.height = 220;
      spr.position.set(12 + (i % 2) * 232, 12 + ((i / 2) | 0) * 232);
      this.container.addChild(spr);
    }
  }

  destroy(): void {
    this.target?.destroy();
    this.mesh?.destroy(true);
    this.container.destroy({ children: true });
  }
}
