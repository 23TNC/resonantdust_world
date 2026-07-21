//! Blitter — composite a source {@link Texture} (or a sub-frame of one) into a PIXEL sub-rect of a
//! target {@link RenderTarget}. This is the sprite→atlas-page composite Pixi did via `renderer.render
//! ({ container: sprite, target })`; the engine has no scene graph, so it's one textured-quad draw
//! placed by an NDC dest rect + a source-UV rect (the same `uDst`/`uSrcRect` shader the SquareCache's
//! slot blit uses). `overwrite` (blend "none") copies verbatim — the atlas packs straight bytes.

import { Program } from "./program";
import { Geometry } from "./geometry";
import type { Texture } from "./texture";
import type { TexFrame } from "./texFrame";
import type { Renderer } from "./renderer";
import type { RenderTarget } from "./renderTarget";

const BLIT_VERT = /* glsl */ `#version 300 es
in vec2 aPosition;             // unit quad 0..1
uniform vec4 uDst;             // dest NDC rect: x0, y0, w, h
uniform vec4 uSrcRect;         // source UV rect: u0, v0, w, h
out vec2 vUV;
void main() {
  vUV = uSrcRect.xy + aPosition * uSrcRect.zw;
  vec2 ndc = uDst.xy + aPosition * uDst.zw;
  gl_Position = vec4(ndc, 0.0, 1.0);
}
`;
const BLIT_FRAG = /* glsl */ `#version 300 es
precision highp float;
in vec2 vUV;
uniform sampler2D uSrc;
out vec4 fragColor;
void main() { fragColor = texture(uSrc, vUV); }
`;

export class Blitter {
  private readonly program: Program;
  private readonly quad: Geometry;

  constructor(private readonly renderer: Renderer) {
    const gl = renderer.gl;
    this.program = new Program(gl, BLIT_VERT, BLIT_FRAG, "blitter");
    this.quad = new Geometry(gl, this.program, {
      aPosition: { data: new Float32Array([0, 0, 1, 0, 1, 1, 0, 0, 1, 1, 0, 1]), size: 2 },
    });
  }

  /** Copy `src` into `target` at pixel rect (dx, dy, dw, dh). `srcFrame`, if given, restricts the
   *  sampled region to that frame's UV sub-rect (else the whole `src`). Overwrites (blend "none"). */
  blit(target: RenderTarget, src: Texture, dx: number, dy: number, dw: number, dh: number, srcFrame?: TexFrame): void {
    // Pixel dest rect → the target's NDC (y-up; the atlas stores upright and the resolver's frames
    // are read back at the same orientation, so no flip here).
    const x0 = (dx / target.width) * 2 - 1;
    const y0 = (dy / target.height) * 2 - 1;
    const w = (dw / target.width) * 2;
    const h = (dh / target.height) * 2;
    const uv = srcFrame ? srcFrame.uvRect() : [0, 0, 1, 1];
    this.renderer.draw({
      program: this.program,
      geometry: this.quad,
      target,
      blend: "none",
      textures: { uSrc: src },
      uniforms: (p) => {
        p.uVec4("uDst", x0, y0, w, h);
        p.uVec4("uSrcRect", uv[0], uv[1], uv[2], uv[3]);
      },
    });
  }

  destroy(): void {
    this.quad.destroy();
    this.program.destroy();
  }
}
