//! Geometry — a VAO with per-vertex and per-instance attributes + an optional index buffer. Handles float
//! and integer attributes (`vertexAttribIPointer` for the latter) and instancing (`vertexAttribDivisor`).
//! Attribute locations resolve from the {@link Program} it's built for. Buffers are updatable in place
//! (`update`) for the per-frame data the caster-lut cast will push.

import { Program } from "./program";

export interface AttribSpec {
  data: Float32Array | Uint32Array | Int32Array;
  /** Components per vertex (1..4). */
  size: number;
  /** Advance once per instance instead of per vertex. */
  instanced?: boolean;
  /** Integer attribute (bound with `vertexAttribIPointer`, read as `in uint`/`in int`). Default float. */
  integer?: boolean;
}

interface Buf {
  handle: WebGLBuffer;
  spec: AttribSpec;
  loc: number;
}

export class Geometry {
  readonly count: number;
  /** Instances to draw (0 = non-instanced). Mutable so a per-frame cast can vary the count after
   *  `update()`-ing the instance buffers (the caster-lut / shadow cast pushes a new pair count each frame). */
  instanceCount: number;
  private readonly gl: WebGL2RenderingContext;
  readonly vao: WebGLVertexArrayObject;
  private readonly bufs = new Map<string, Buf>();
  private indexBuffer: WebGLBuffer | null = null;
  private indexCount = 0;

  constructor(gl: WebGL2RenderingContext, program: Program, attributes: Record<string, AttribSpec>, indices?: Uint32Array, instanceCount = 0) {
    this.gl = gl;
    this.instanceCount = instanceCount;
    this.vao = gl.createVertexArray()!;
    gl.bindVertexArray(this.vao);

    let vertCount = 0;
    for (const [name, spec] of Object.entries(attributes)) {
      const loc = program.attribLoc(name);
      const handle = gl.createBuffer()!;
      gl.bindBuffer(gl.ARRAY_BUFFER, handle);
      gl.bufferData(gl.ARRAY_BUFFER, spec.data, gl.DYNAMIC_DRAW);
      if (loc >= 0) {
        gl.enableVertexAttribArray(loc);
        if (spec.integer) {
          const type = spec.data instanceof Uint32Array ? gl.UNSIGNED_INT : gl.INT;
          gl.vertexAttribIPointer(loc, spec.size, type, 0, 0);
        } else {
          gl.vertexAttribPointer(loc, spec.size, gl.FLOAT, false, 0, 0);
        }
        if (spec.instanced) gl.vertexAttribDivisor(loc, 1);
      }
      this.bufs.set(name, { handle, spec, loc });
      if (!spec.instanced) vertCount = Math.max(vertCount, spec.data.length / spec.size);
    }

    if (indices) {
      this.indexBuffer = gl.createBuffer();
      gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, this.indexBuffer);
      gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, indices, gl.DYNAMIC_DRAW);
      this.indexCount = indices.length;
    }
    this.count = this.indexCount || vertCount;
    gl.bindVertexArray(null);
  }

  /** Replace an attribute buffer's data in place (same or fewer elements). */
  update(name: string, data: Float32Array | Uint32Array | Int32Array): void {
    const b = this.bufs.get(name);
    if (!b) throw new Error(`[gl] no attribute '${name}'`);
    this.gl.bindBuffer(this.gl.ARRAY_BUFFER, b.handle);
    this.gl.bufferData(this.gl.ARRAY_BUFFER, data, this.gl.DYNAMIC_DRAW);
  }

  /** Draw (TRIANGLES by default). Instanced when `instanceCount > 0`. `count` overrides the vertex
   *  count for a non-indexed draw (the scatter pass draws exactly N command points per flush). */
  draw(mode?: number, count?: number): void {
    const gl = this.gl;
    const m = mode ?? gl.TRIANGLES;
    gl.bindVertexArray(this.vao);
    if (this.instanceCount > 0) {
      if (this.indexBuffer) gl.drawElementsInstanced(m, this.count, gl.UNSIGNED_INT, 0, this.instanceCount);
      else gl.drawArraysInstanced(m, 0, this.count, this.instanceCount);
    } else if (this.indexBuffer) {
      gl.drawElements(m, this.count, gl.UNSIGNED_INT, 0);
    } else {
      gl.drawArrays(m, 0, count ?? this.count);
    }
    gl.bindVertexArray(null);
  }

  destroy(): void {
    this.gl.deleteVertexArray(this.vao);
    for (const b of this.bufs.values()) this.gl.deleteBuffer(b.handle);
    if (this.indexBuffer) this.gl.deleteBuffer(this.indexBuffer);
  }
}
