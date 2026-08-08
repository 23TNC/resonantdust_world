//! Program — a compiled + linked GLSL ES 3.00 shader program, with cached uniform/attribute locations and
//! typed setters. Raw ES 3.00 (`#version 300 es`) — the shader GLSL bodies come straight from the pixijs
//! client (already ES 3.00); this is the harness that replaces `compileHighShaderGlProgram`. No dialect
//! shims, no template — what you write is what compiles, and vertex samplers (VTF), `usampler2D`, and
//! `uvec4` outputs Just Work.

export class Program {
  readonly handle: WebGLProgram;
  private readonly gl: WebGL2RenderingContext;
  private readonly uCache = new Map<string, WebGLUniformLocation | null>();
  private readonly aCache = new Map<string, number>();

  constructor(gl: WebGL2RenderingContext, vertex: string, fragment: string, name = "program") {
    this.gl = gl;
    const vs = compile(gl, gl.VERTEX_SHADER, vertex, name + ".vert");
    const fs = compile(gl, gl.FRAGMENT_SHADER, fragment, name + ".frag");
    const p = gl.createProgram()!;
    gl.attachShader(p, vs);
    gl.attachShader(p, fs);
    gl.linkProgram(p);
    if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error(`[gl] link ${name}: ${gl.getProgramInfoLog(p)}`);
    gl.deleteShader(vs);
    gl.deleteShader(fs);
    this.handle = p;
  }

  use(): void {
    this.gl.useProgram(this.handle);
  }

  attribLoc(name: string): number {
    let l = this.aCache.get(name);
    if (l === undefined) {
      l = this.gl.getAttribLocation(this.handle, name);
      this.aCache.set(name, l);
    }
    return l;
  }

  private loc(name: string): WebGLUniformLocation | null {
    let l = this.uCache.get(name);
    if (l === undefined) {
      l = this.gl.getUniformLocation(this.handle, name);
      this.uCache.set(name, l);
    }
    return l;
  }

  uFloat(name: string, v: number): void {
    this.gl.uniform1f(this.loc(name), v);
  }
  uInt(name: string, v: number): void {
    this.gl.uniform1i(this.loc(name), v);
  }
  uVec2(name: string, x: number, y: number): void {
    this.gl.uniform2f(this.loc(name), x, y);
  }
  uVec3(name: string, x: number, y: number, z: number): void {
    this.gl.uniform3f(this.loc(name), x, y, z);
  }
  uVec4(name: string, x: number, y: number, z: number, w: number): void {
    this.gl.uniform4f(this.loc(name), x, y, z, w);
  }
  uVec4Array(name: string, data: Float32Array): void {
    this.gl.uniform4fv(this.loc(name), data);
  }
  uIntArray(name: string, data: Int32Array): void {
    this.gl.uniform1iv(this.loc(name), data);
  }
  uMat3(name: string, m: Float32Array): void {
    this.gl.uniformMatrix3fv(this.loc(name), false, m);
  }
  /** Bind sampler uniform `name` to texture unit `unit`. */
  uSampler(name: string, unit: number): void {
    this.gl.uniform1i(this.loc(name), unit);
  }

  destroy(): void {
    this.gl.deleteProgram(this.handle);
  }
}

function compile(gl: WebGL2RenderingContext, type: number, src: string, name: string): WebGLShader {
  const sh = gl.createShader(type)!;
  gl.shaderSource(sh, src);
  gl.compileShader(sh);
  if (!gl.getShaderParameter(sh, gl.COMPILE_STATUS)) throw new Error(`[gl] compile ${name}: ${gl.getShaderInfoLog(sh)}`);
  return sh;
}
