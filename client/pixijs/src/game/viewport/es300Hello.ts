//! es300-hello experiment — the FIRST hand-crafted GLSL ES 3.00 shader in the client. Everything else is
//! Pixi's high-shader (GLSL ES 1.00, no `uint`/bitwise/`texelFetch`); this proves we can hand-write a raw
//! `#version 300 es` program and render it into the viewport, using syntax ES 1.00 can't compile.
//!
//! Pixi v8's `GlProgram` detects `#version 300 es` in the FRAGMENT and processes both stages as ES 3.00
//! (strips the directive, re-inserts it as line 1, skips the WebGL1 `#define in varying` shims) — so we
//! just supply raw source. A WebGL2 context runs this ES 3.00 program alongside the rest of the ES 1.00
//! scene (es300-hello I-4). The vertex is a clip-space passthrough (a full-screen quad at [-1,1], no
//! projection needed); the fragment draws an opaque cyan checkerboard from `uint`/bitwise math — the
//! constructs that throw `'uint' : undeclared identifier` under ES 1.00. Cyan squares over the scene ⇒ ES
//! 3.00 is live. Foundation for caster-lut C5 (the vertex-texture-fetch GPU cast).

import { Buffer, BufferUsage, Container, Geometry, GlProgram, Mesh, Shader, Texture } from "pixi.js";

const VERT = /* glsl */ `#version 300 es
in vec2 aPosition;
void main() {
  gl_Position = vec4(aPosition, 0.0, 1.0); // aPosition already clip-space → full-screen, camera-independent
}
`;

// ES-3.00-ONLY syntax (uint / uvec2 / bitwise ^ & / the `1u` literal / a user-declared `out`): ES 1.00
// rejects every one of these at compile time, so a rendered checker is unambiguous proof of ES 3.00.
const FRAG = /* glsl */ `#version 300 es
precision highp float;
out vec4 fragColor;
void main() {
  uvec2 p = uvec2(gl_FragCoord.xy) / 24u;
  uint checker = (p.x ^ p.y) & 1u;
  fragColor = checker == 1u ? vec4(0.15, 0.75, 1.0, 1.0) : vec4(0.0); // opaque cyan squares, transparent gaps
}
`;

let program: GlProgram | null = null;
function prog(): GlProgram {
  if (!program) program = GlProgram.from({ vertex: VERT, fragment: FRAG, name: "es300-hello" });
  return program;
}

/** The mesh pipe wants a `TextureShader` (a `texture` accessor); this program samples nothing, so it's a
 *  no-op stub returning EMPTY (never bound — the program declares no sampler). */
class Es300Shader extends Shader {
  get texture(): Texture {
    return Texture.EMPTY;
  }
  set texture(_v: Texture) {
    /* no-op */
  }
}

/** A toggleable full-viewport ES 3.00 checkerboard mesh. Lazily built on first enable. */
export class Es300Hello {
  readonly container = new Container();
  private mesh: Mesh<Geometry> | null = null;
  private running = false;

  constructor() {
    this.container.visible = false;
  }

  toggle(): boolean {
    this.running = !this.running;
    if (this.running && !this.mesh) {
      const geo = new Geometry({
        attributes: {
          aPosition: { buffer: new Buffer({ data: new Float32Array([-1, -1, 1, -1, 1, 1, -1, 1]), usage: BufferUsage.VERTEX }), format: "float32x2" },
        },
        indexBuffer: new Buffer({ data: new Uint32Array([0, 1, 2, 0, 2, 3]), usage: BufferUsage.INDEX }),
      });
      this.mesh = new Mesh<Geometry>({ geometry: geo, shader: new Es300Shader({ glProgram: prog(), resources: {} }) });
      this.container.addChild(this.mesh);
    }
    this.container.visible = this.running;
    return this.running;
  }

  destroy(): void {
    this.mesh?.destroy(true);
    this.container.destroy({ children: true });
  }
}
