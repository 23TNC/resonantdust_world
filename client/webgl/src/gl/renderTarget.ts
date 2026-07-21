//! RenderTarget — an FBO with N colour attachments (MRT), any of the {@link Texture} formats (incl. INTEGER
//! + float). Unlike Pixi, we **always call `gl.drawBuffers`** so every attachment is written, and we expose
//! both a float `clear` and an integer `clearInt` (`clearBufferuiv`) so an `RGBA8UI` target clears correctly.
//! This is the piece the shadow bitfield + MRT bakes need that Pixi couldn't do.

import { Texture, type TexFormat } from "./texture";

export interface RenderTargetOptions {
  width: number;
  height: number;
  /** One format per colour attachment. `[fmt]` = single target; `[a,b,…]` = MRT. */
  formats: TexFormat[];
}

export class RenderTarget {
  readonly fbo: WebGLFramebuffer;
  readonly textures: Texture[];
  width: number;
  height: number;
  private readonly gl: WebGL2RenderingContext;
  private readonly formats: TexFormat[];
  private readonly integer: boolean;

  constructor(gl: WebGL2RenderingContext, opts: RenderTargetOptions) {
    this.gl = gl;
    this.width = opts.width;
    this.height = opts.height;
    this.formats = opts.formats;
    this.integer = opts.formats.every((f) => f === "rgba8uint" || f === "rgba32uint");
    this.fbo = gl.createFramebuffer()!;
    this.textures = opts.formats.map((format) => new Texture(gl, { width: opts.width, height: opts.height, format }));
    gl.bindFramebuffer(gl.FRAMEBUFFER, this.fbo);
    const buffers: number[] = [];
    this.textures.forEach((tex, i) => {
      gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0 + i, gl.TEXTURE_2D, tex.handle, 0);
      buffers.push(gl.COLOR_ATTACHMENT0 + i);
    });
    gl.drawBuffers(buffers); // enable ALL attachments (Pixi never did this — the MRT/integer gap)
    const status = gl.checkFramebufferStatus(gl.FRAMEBUFFER);
    if (status !== gl.FRAMEBUFFER_COMPLETE) throw new Error(`[gl] RenderTarget incomplete: 0x${status.toString(16)} (formats ${opts.formats.join(",")})`);
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
  }

  /** Bind this target's FBO + set the viewport. Re-asserts drawBuffers (cheap; per-FBO but safe). */
  bind(): void {
    const gl = this.gl;
    gl.bindFramebuffer(gl.FRAMEBUFFER, this.fbo);
    gl.viewport(0, 0, this.width, this.height);
  }

  /** Float clear (unorm/float attachments). */
  clear(r: number, g: number, b: number, a: number): void {
    this.bind();
    this.gl.clearColor(r, g, b, a);
    this.gl.clear(this.gl.COLOR_BUFFER_BIT);
  }

  /** Integer clear (RGBA8UI/RGBA32UI attachment 0). */
  clearInt(r: number, g: number, b: number, a: number): void {
    this.bind();
    this.gl.clearBufferuiv(this.gl.COLOR, 0, new Uint32Array([r, g, b, a]));
  }

  destroy(): void {
    this.gl.deleteFramebuffer(this.fbo);
    for (const t of this.textures) t.destroy();
  }
}
