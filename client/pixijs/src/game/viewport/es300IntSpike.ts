//! caster-lut C5a spike — proving the ES 3.00 GPU-cast techniques. FINDINGS (2026-07-20):
//!   ✅ Instancing + per-instance attributes, float render targets, and `texelFetch` in the FRAGMENT (+ a
//!      full-screen `texelFetch` display) all work through a raw ES 3.00 `GlProgram` mesh — this file shows
//!      5 distinct colour-quads (`/inttest`), positioned per-instance from `aIndex`.
//!   ❌ VERTEX-TEXTURE-FETCH via a raw `GlProgram` returned 0 — the texture resource wasn't bound to the raw
//!      program's VERTEX sampler (`texelFetch(uData)` in the vertex read zeros; the quad collapsed to clip
//!      (0,0)). Needs the high-shader ES 3.00 path (proven texture binding) + a VTF vertex bit, or raw GL.
//!   ❌ INTEGER render target (`RGBA8UI`, `uvec4` out, `usampler2D` read) → `GL_INVALID_OPERATION` through
//!      Pixi's mesh render. Needs raw-GL FBO + draw (own `clearBufferuiv`/state), bypassing the mesh pipe.
//! See caster-lut issues I-10 / I-11 + the C5 re-scope. Left in ISOLATION MODE (float RT, aIndex-positioned)
//! as the working proof of the parts that DO work.

import { Buffer, BufferUsage, Container, Geometry, GlProgram, Mesh, RenderTarget, Shader, Texture, TextureSource, BufferImageSource, type Renderer } from "pixi.js";

const N = 5;
/** 5 records (x, y, bit, _) — quad centres in clip space + which bit each sets. */
const RECORDS = new Float32Array([
  -0.6, 0.0, 0, 0,
  -0.3, 0.0, 1, 0,
  0.0, 0.0, 2, 0,
  0.3, 0.0, 3, 0,
  0.6, 0.0, 4, 0,
]);

// ISOLATION 2: position from aIndex DIRECTLY (no VTF) — does instancing + the per-instance attribute work?
const CAST_VERT = /* glsl */ `#version 300 es
in vec2 aPosition;               // unit quad -1..1
in float aIndex;                 // per-instance record index
uniform highp sampler2D uData;   // (bound but unused in this isolation step)
flat out uint vBit;
void main() {
  vBit = uint(aIndex + 0.5);
  float x = -0.6 + aIndex * 0.3;
  gl_Position = vec4(vec2(x, 0.0) + aPosition * 0.12, 0.0, 1.0);
}
`;
// ISOLATION MODE: write a FLOAT colour to a normal target to prove VTF + texelFetch + instancing on their
// own (the integer-RT path hit GL_INVALID_OPERATION through Pixi — isolating it).
const CAST_FRAG = /* glsl */ `#version 300 es
precision highp float;
flat in uint vBit;
out vec4 oCol;
void main() {
  vec3 pal[5] = vec3[5](vec3(1.0,0.25,0.25), vec3(0.25,1.0,0.30), vec3(0.30,0.55,1.0), vec3(1.0,0.95,0.25), vec3(1.0,0.35,1.0));
  oCol = vec4(pal[vBit], 1.0);
}
`;

const DISP_VERT = /* glsl */ `#version 300 es
in vec2 aPosition;
out vec2 vUV;
void main() { vUV = aPosition * 0.5 + 0.5; gl_Position = vec4(aPosition, 0.0, 1.0); }
`;
const DISP_FRAG = /* glsl */ `#version 300 es
precision highp float;
in vec2 vUV;
uniform highp sampler2D uInt;    // ISOLATION: normal sampler on the float target
out vec4 fragColor;
void main() {
  vec4 c = texelFetch(uInt, ivec2(vUV * vec2(textureSize(uInt, 0))), 0);
  fragColor = length(c.rgb) < 0.01 ? vec4(0.0) : vec4(c.rgb, 1.0);
}
`;

/** Mesh pipe wants a TextureShader — these programs bind their own samplers, so a no-op stub. */
class StubShader extends Shader {
  get texture(): Texture {
    return Texture.EMPTY;
  }
  set texture(_v: Texture) {
    /* no-op */
  }
}

export class Es300IntSpike {
  readonly container = new Container();
  private intTarget: RenderTarget | null = null;
  private intTex: Texture | null = null;
  private castMesh: Mesh<Geometry> | null = null;
  private running = false;

  constructor() {
    this.container.visible = false;
  }

  toggle(): boolean {
    this.running = !this.running;
    this.container.visible = this.running;
    return this.running;
  }

  render(renderer: Renderer): void {
    if (!this.running) return;
    if (!this.intTarget) this.build();
    const gl = (renderer as unknown as { gl: WebGL2RenderingContext }).gl;
    // Cast the 5 bit-quads into the integer target. Clear it with an INTEGER clear (Pixi's float clear won't
    // touch a uint attachment), then draw with blend off (integer targets can't blend).
    void gl; // (ISOLATION) normal float clear — integer clearBufferuiv comes back with the integer target
    renderer.render({ container: this.castMesh!, target: this.intTarget!, clear: true, clearColor: [0, 0, 0, 1] });
  }

  private build(): void {
    const dataTex = new Texture({ source: new BufferImageSource({ resource: RECORDS, width: N, height: 1, format: "rgba32float", scaleMode: "nearest" }) });
    const intSource = new TextureSource({ width: 640, height: 360, format: "rgba8unorm", scaleMode: "nearest" });
    this.intTarget = new RenderTarget({ colorTextures: [intSource] });
    this.intTex = new Texture({ source: intSource });

    const castGeo = new Geometry({
      attributes: {
        aPosition: { buffer: new Buffer({ data: new Float32Array([-1, -1, 1, -1, 1, 1, -1, 1]), usage: BufferUsage.VERTEX }), format: "float32x2" },
        aIndex: { buffer: new Buffer({ data: new Float32Array([0, 1, 2, 3, 4]), usage: BufferUsage.VERTEX }), format: "float32", instance: true },
      },
      indexBuffer: new Buffer({ data: new Uint32Array([0, 1, 2, 0, 2, 3]), usage: BufferUsage.INDEX }),
      instanceCount: N,
    });
    const castShader = new StubShader({
      glProgram: GlProgram.from({ vertex: CAST_VERT, fragment: CAST_FRAG, name: "int-spike-cast" }),
      resources: { uData: dataTex.source, uDataSampler: dataTex.source.style },
    });
    this.castMesh = new Mesh<Geometry>({ geometry: castGeo, shader: castShader });
    this.castMesh.blendMode = "none";

    const dispGeo = new Geometry({
      attributes: { aPosition: { buffer: new Buffer({ data: new Float32Array([-1, -1, 1, -1, 1, 1, -1, 1]), usage: BufferUsage.VERTEX }), format: "float32x2" } },
      indexBuffer: new Buffer({ data: new Uint32Array([0, 1, 2, 0, 2, 3]), usage: BufferUsage.INDEX }),
    });
    const dispShader = new StubShader({
      glProgram: GlProgram.from({ vertex: DISP_VERT, fragment: DISP_FRAG, name: "int-spike-disp" }),
      resources: { uInt: this.intTex.source, uIntSampler: this.intTex.source.style },
    });
    this.container.addChild(new Mesh<Geometry>({ geometry: dispGeo, shader: dispShader }));
  }

  destroy(): void {
    this.intTarget?.destroy();
    this.castMesh?.destroy(true);
    this.container.destroy({ children: true });
  }
}
