//! TexFrame — a UV sub-frame onto a shared atlas-page {@link Texture}. The engine keeps whole GL
//! textures (pages); the caller (the texture resolver) tracks WHICH rectangle of a page a given
//! sprite occupies. This replaces Pixi's `Texture { source, frame }` — a value object (no GL
//! resource), so packing a sprite into an atlas returns a `TexFrame`, and the baker reads its
//! `uvRect()` for the sampler `[offU, offV, scaleU, scaleV]` the merged bake expects.

import type { Texture } from "./texture";

export class TexFrame {
  constructor(
    /** The atlas page this frame lives on. */
    readonly source: Texture,
    /** The sub-rectangle in page PIXELS. */
    readonly x: number,
    readonly y: number,
    readonly w: number,
    readonly h: number,
  ) {}

  /** Whole-page frame (identity) over a texture. */
  static whole(t: Texture): TexFrame {
    return new TexFrame(t, 0, 0, t.width, t.height);
  }

  /** The bake's `[offU, offV, scaleU, scaleV]` — the frame as a 0..1 UV rect on its page (a
   *  `Float32Array` to match the bake shader's rect uniforms directly). */
  uvRect(): Float32Array {
    return new Float32Array([this.x / this.source.width, this.y / this.source.height, this.w / this.source.width, this.h / this.source.height]);
  }
}
