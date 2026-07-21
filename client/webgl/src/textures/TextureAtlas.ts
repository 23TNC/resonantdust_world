//! One GPU-backed atlas page (webgl port): a square {@link RenderTarget} whose space is handed out
//! by a {@link MaxRectsPacker}. Adding a texture packs a rect, composites the source into that rect
//! via the {@link Blitter}, and returns a {@link TexFrame} onto this page's texture — so every
//! sub-texture shares one page and batches in a single draw.
//!
//! Ported from the pixijs `RenderTexture` + `Sprite` composite: the page is a `RenderTarget`, and the
//! sprite-into-RT render is one `Blitter.blit` (blend "none" = overwrite). Rects never overlap and the
//! page is cleared transparent once, so the overwrite copies any source verbatim — the premultiply
//! (colour vs straight-alpha data maps) lives in the source `Texture`'s upload, not here.

import { RenderTarget, TexFrame, type Texture, type Renderer, type Blitter } from "../gl";
import { MaxRectsPacker } from "./MaxRectsPacker";

export class TextureAtlas {
  /** The page's backing target. Sub-frames reference its texture. */
  readonly page: RenderTarget;

  private readonly blitter: Blitter;
  private readonly packer: MaxRectsPacker;
  private regionCount = 0;

  constructor(renderer: Renderer, blitter: Blitter, size: number) {
    this.blitter = blitter;
    // nearest + integer frames → exact sampling, no edge bleed between packed neighbours.
    this.page = new RenderTarget(renderer.gl, { width: size, height: size, formats: ["rgba8unorm"] });
    this.packer = new MaxRectsPacker(size, size);
    // Initialise the whole page transparent so a not-yet-packed area never shows GPU garbage;
    // every later add composites with overwrite (blend "none"), never clearing.
    this.page.clear(0, 0, 0, 0);
  }

  /** Number of textures packed into this page. */
  get count(): number {
    return this.regionCount;
  }
  /** Fraction of the page's area handed out (0–1). */
  get occupancy(): number {
    return this.packer.occupancy;
  }

  /** Pack `source` (scaled to `width × height`) into a free rect, drawing it into the page.
   *  `padding` reserves a right/bottom gutter so neighbours can't sample into each other. Returns the
   *  framed sub-texture, or `null` if the page is full. */
  add(source: Texture, width: number, height: number, padding: number): TexFrame | null {
    const rect = this.packer.insert(width + padding, height + padding);
    if (!rect) return null;
    this.blitter.blit(this.page, source, rect.x, rect.y, width, height);
    this.regionCount++;
    return new TexFrame(this.page.textures[0], rect.x, rect.y, width, height);
  }

  destroy(): void {
    this.page.destroy();
  }
}
