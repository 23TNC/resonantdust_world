//! client/webgl — W1 scaffold. Our OWN WebGL2 context + frame loop, no PixiJS. A cleared canvas + a single
//! (per-vertex-coloured) triangle proves the context, the RAF loop, and the vite/TS build are ours
//! end-to-end. The engine core (Program / Geometry / Texture / RenderTarget / Renderer) lands in W2; this is
//! deliberately the barest possible raw-GL to prove the foundation.

const VERT = /* glsl */ `#version 300 es
in vec2 aPosition;
in vec3 aColor;
out vec3 vColor;
void main() {
  vColor = aColor;
  gl_Position = vec4(aPosition, 0.0, 1.0);
}
`;
const FRAG = /* glsl */ `#version 300 es
precision highp float;
in vec3 vColor;
out vec4 fragColor;
void main() { fragColor = vec4(vColor, 1.0); }
`;

function compile(gl: WebGL2RenderingContext, type: number, src: string): WebGLShader {
  const sh = gl.createShader(type)!;
  gl.shaderSource(sh, src);
  gl.compileShader(sh);
  if (!gl.getShaderParameter(sh, gl.COMPILE_STATUS)) throw new Error("[webgl] compile: " + gl.getShaderInfoLog(sh));
  return sh;
}
function link(gl: WebGL2RenderingContext, vs: string, fs: string): WebGLProgram {
  const p = gl.createProgram()!;
  gl.attachShader(p, compile(gl, gl.VERTEX_SHADER, vs));
  gl.attachShader(p, compile(gl, gl.FRAGMENT_SHADER, fs));
  gl.linkProgram(p);
  if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error("[webgl] link: " + gl.getProgramInfoLog(p));
  return p;
}

function main(): void {
  const host = document.getElementById("app")!;
  const canvas = document.createElement("canvas");
  host.appendChild(canvas);
  const gl = canvas.getContext("webgl2", { alpha: false, antialias: false, premultipliedAlpha: true, powerPreference: "high-performance" });
  if (!gl) throw new Error("[webgl] WebGL2 not available");

  const prog = link(gl, VERT, FRAG);
  const aPos = gl.getAttribLocation(prog, "aPosition");
  const aCol = gl.getAttribLocation(prog, "aColor");

  // One triangle: interleaved [pos.xy, colour.rgb] per vertex.
  const data = new Float32Array([
    0.0, 0.6, 1.0, 0.25, 0.25,
    -0.6, -0.5, 0.25, 1.0, 0.3,
    0.6, -0.5, 0.3, 0.55, 1.0,
  ]);
  const vao = gl.createVertexArray();
  gl.bindVertexArray(vao);
  const buf = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, buf);
  gl.bufferData(gl.ARRAY_BUFFER, data, gl.STATIC_DRAW);
  const stride = 5 * 4;
  gl.enableVertexAttribArray(aPos);
  gl.vertexAttribPointer(aPos, 2, gl.FLOAT, false, stride, 0);
  gl.enableVertexAttribArray(aCol);
  gl.vertexAttribPointer(aCol, 3, gl.FLOAT, false, stride, 2 * 4);
  gl.bindVertexArray(null);

  function resize(): void {
    const dpr = window.devicePixelRatio || 1;
    const w = Math.max(1, Math.floor(canvas.clientWidth * dpr));
    const h = Math.max(1, Math.floor(canvas.clientHeight * dpr));
    if (canvas.width !== w || canvas.height !== h) {
      canvas.width = w;
      canvas.height = h;
    }
  }

  function frame(): void {
    resize();
    gl!.viewport(0, 0, canvas.width, canvas.height);
    gl!.clearColor(0.06, 0.08, 0.1, 1.0);
    gl!.clear(gl!.COLOR_BUFFER_BIT);
    gl!.useProgram(prog);
    gl!.bindVertexArray(vao);
    gl!.drawArrays(gl!.TRIANGLES, 0, 3);
    gl!.bindVertexArray(null);
    requestAnimationFrame(frame);
  }
  requestAnimationFrame(frame);
  // eslint-disable-next-line no-console
  console.log("[webgl] W1 hello: WebGL2 context + loop up (", gl.getParameter(gl.VERSION), ")");
}

main();
