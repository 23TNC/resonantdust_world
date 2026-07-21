//! client/webgl — W2 test scene. Exercises the engine core end-to-end, proving the SIX techniques the
//! shadow/lighting work needs — the exact set that failed under Pixi (caster-lut I-10/I-11):
//!   1. textured quad          (Texture upload + sampling)               → top-left checker
//!   2. render-to-target       (draw into a RenderTarget, sample it)     → every result below is one
//!   3. MRT                    (2 outputs → 2 attachments)               → two quads, bottom
//!   4. integer target         (RGBA8UI write + usampler2D + uint read)  → centre colour-quads
//!   5. instancing             (per-instance attribute)                  → 5 quads
//!   6. vertex-texture-fetch   (vertex reads a data texture)             → the 5 quads are POSITIONED by it
//! Five decoded colour-quads + a checker + two MRT quads ⇒ the engine works. Then W3 copies the client onto it.

import { Renderer, Program, Geometry, Texture, RenderTarget } from "./gl";

// ── shaders ─────────────────────────────────────────────────────────────────────
// Cast: instanced quads, each reading its record from an RGBA32F data texture in the VERTEX stage (VTF),
// writing its bit into an INTEGER target.
const CAST_VERT = /* glsl */ `#version 300 es
in vec2 aPosition;              // unit quad (triangle strip)
in float aIndex;               // per-instance record index
uniform highp sampler2D uData; // RGBA32F records — sampled here in the VERTEX stage (VTF)
flat out uint vBit;
void main() {
  vec4 rec = texelFetch(uData, ivec2(int(aIndex + 0.5), 0), 0);
  vBit = uint(rec.z + 0.5);
  gl_Position = vec4(rec.xy + aPosition * 0.12, 0.0, 1.0);
}
`;
const CAST_FRAG = /* glsl */ `#version 300 es
precision highp float;
flat in uint vBit;
out uvec4 oBits;               // INTEGER output → RGBA8UI target
void main() { oBits = uvec4(1u << vBit, 0u, 0u, 0u); }
`;

// Integer display: read the RGBA8UI target via usampler2D, decode the bits with real uint bitwise.
const INT_DISP_FRAG = /* glsl */ `#version 300 es
precision highp float;
in vec2 vUV;
uniform highp usampler2D uInt;
out vec4 fragColor;
void main() {
  uint n = texelFetch(uInt, ivec2(vUV * vec2(textureSize(uInt, 0))), 0).r;
  vec3 acc = vec3(0.0);
  if ((n & 1u)  != 0u) acc += vec3(1.0, 0.25, 0.25);
  if ((n & 2u)  != 0u) acc += vec3(0.25, 1.0, 0.30);
  if ((n & 4u)  != 0u) acc += vec3(0.30, 0.55, 1.0);
  if ((n & 8u)  != 0u) acc += vec3(1.0, 0.95, 0.25);
  if ((n & 16u) != 0u) acc += vec3(1.0, 0.35, 1.0);
  fragColor = length(acc) < 0.01 ? vec4(0.0) : vec4(clamp(acc, 0.0, 1.0), 1.0);
}
`;

// A quad drawn at a clip-space rect (uPos: x,y,w,h) sampling a texture — for the checker + MRT results.
const TEX_VERT = /* glsl */ `#version 300 es
in vec2 aPosition;             // unit quad 0..1
in vec2 aUV;
uniform vec4 uRect;            // x, y, w, h in clip space
out vec2 vUV;
void main() { vUV = aUV; gl_Position = vec4(uRect.xy + aPosition * uRect.zw, 0.0, 1.0); }
`;
const TEX_FRAG = /* glsl */ `#version 300 es
precision highp float;
in vec2 vUV;
uniform sampler2D uTex;
out vec4 fragColor;
void main() { fragColor = vec4(texture(uTex, vUV).rgb, 1.0); }
`;
const FS_VERT = /* glsl */ `#version 300 es
in vec2 aPosition;
out vec2 vUV;
void main() { vUV = aPosition * 0.5 + 0.5; gl_Position = vec4(aPosition, 0.0, 1.0); }
`;

// MRT: two outputs → two attachments.
const MRT_VERT = /* glsl */ `#version 300 es
in vec2 aPosition;
out vec2 vUV;
void main() { vUV = aPosition * 0.5 + 0.5; gl_Position = vec4(aPosition, 0.0, 1.0); }
`;
const MRT_FRAG = /* glsl */ `#version 300 es
precision highp float;
in vec2 vUV;
layout(location = 0) out vec4 o0;
layout(location = 1) out vec4 o1;
void main() {
  o0 = vec4(vUV.x, 0.3, 0.3, 1.0);   // attachment 0: red ramp
  o1 = vec4(0.3, 0.3, vUV.y, 1.0);   // attachment 1: blue ramp
}
`;

function main(): void {
  const host = document.getElementById("app")!;
  const r = new Renderer();
  host.appendChild(r.canvas);
  const gl = r.gl;

  // Programs
  const castProg = new Program(gl, CAST_VERT, CAST_FRAG, "cast");
  const intDisp = new Program(gl, FS_VERT, INT_DISP_FRAG, "int-disp");
  const texProg = new Program(gl, TEX_VERT, TEX_FRAG, "tex");
  const mrtProg = new Program(gl, MRT_VERT, MRT_FRAG, "mrt");

  // Data texture: 5 records (x, y, bit, _) — quad centres in clip space + which bit each sets.
  const dataTex = new Texture(gl, {
    width: 5, height: 1, format: "rgba32float",
    data: new Float32Array([-0.6, 0.0, 0, 0, -0.3, 0.0, 1, 0, 0.0, 0.0, 2, 0, 0.3, 0.0, 3, 0, 0.6, 0.0, 4, 0]),
  });
  // A 4×4 checker texture (proves upload + sampling).
  const checker = new Uint8Array(4 * 4 * 4);
  for (let i = 0; i < 16; i++) {
    const on = ((i & 1) ^ ((i >> 2) & 1)) === 1;
    checker.set(on ? [230, 180, 60, 255] : [40, 60, 120, 255], i * 4);
  }
  const checkerTex = new Texture(gl, { width: 4, height: 4, format: "rgba8unorm", data: checker });

  // Targets
  const intTarget = new RenderTarget(gl, { width: 640, height: 360, formats: ["rgba8uint"] });
  const mrtTarget = new RenderTarget(gl, { width: 256, height: 256, formats: ["rgba8unorm", "rgba8unorm"] });

  // Geometry
  const castGeo = new Geometry(gl, castProg, {
    aPosition: { data: new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), size: 2 },
    aIndex: { data: new Float32Array([0, 1, 2, 3, 4]), size: 1, instanced: true },
  }, undefined, 5);
  const fsQuad = (p: Program) => new Geometry(gl, p, { aPosition: { data: new Float32Array([-1, -1, 1, -1, 1, 1, -1, 1]), size: 2 } }, new Uint32Array([0, 1, 2, 0, 2, 3]));
  const intQuad = fsQuad(intDisp);
  const mrtQuad = fsQuad(mrtProg);
  // Rect quad (0..1) with uv for the tex program.
  const rectQuad = new Geometry(gl, texProg, {
    aPosition: { data: new Float32Array([0, 0, 1, 0, 1, 1, 0, 1]), size: 2 },
    aUV: { data: new Float32Array([0, 0, 1, 0, 1, 1, 0, 1]), size: 2 },
  }, new Uint32Array([0, 1, 2, 0, 2, 3]));

  function frame(): void {
    r.resize();
    r.clearScreen(0.06, 0.08, 0.1, 1.0);

    // Test 3: MRT — two outputs into a 2-attachment target.
    r.draw({ program: mrtProg, geometry: mrtQuad, target: mrtTarget, clear: [0, 0, 0, 1] });
    // Tests 4/5/6: cast 5 bit-quads into the integer target via VTF + instancing.
    intTarget.clearInt(0, 0, 0, 0);
    r.draw({ program: castProg, geometry: castGeo, target: intTarget, textures: { uData: dataTex }, blend: "none", mode: gl.TRIANGLE_STRIP });

    // Display to screen.
    r.draw({ program: intDisp, geometry: intQuad, textures: { uInt: intTarget.textures[0] }, blend: "normal" }); // centre colour-quads
    r.draw({ program: texProg, geometry: rectQuad, textures: { uTex: checkerTex }, uniforms: (p) => p.uVec4("uRect", -0.95, 0.55, 0.3, 0.3) }); // top-left checker
    r.draw({ program: texProg, geometry: rectQuad, textures: { uTex: mrtTarget.textures[0] }, uniforms: (p) => p.uVec4("uRect", -0.95, -0.95, 0.3, 0.3) }); // MRT 0
    r.draw({ program: texProg, geometry: rectQuad, textures: { uTex: mrtTarget.textures[1] }, uniforms: (p) => p.uVec4("uRect", -0.6, -0.95, 0.3, 0.3) }); // MRT 1
    requestAnimationFrame(frame);
  }
  requestAnimationFrame(frame);
  // eslint-disable-next-line no-console
  console.log("[webgl] W2 engine core up — testing textured-quad / render-to-target / MRT / integer / instancing / VTF");
}

main();
