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

/** A sub-rect composite (P5 ingest scale/clip/re-centre): draw the source's `(sx,sy,sw,sh)` px
 *  sub-rect into the frame's `(dx,dy,dw,dh)` px sub-rect; the rest of the frame stays the page's
 *  cleared transparency. */
export interface AtlasDraw {
  dx: number; dy: number; dw: number; dh: number;
  sx: number; sy: number; sw: number; sh: number;
}

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
   *  `padding` reserves a right/bottom gutter so neighbours can't sample into each other. `draw`
   *  composites a source SUB-rect into a frame sub-rect instead of the whole frame (P5 ingest
   *  scale/clip/re-centre). Returns the framed sub-texture, or `null` if the page is full. */
  add(source: Texture, width: number, height: number, padding: number, draw?: AtlasDraw): TexFrame | null {
    const rect = this.packer.insert(width + padding, height + padding);
    if (!rect) return null;
    if (draw) {
      const srcFrame = new TexFrame(source, draw.sx, draw.sy, draw.sw, draw.sh);
      this.blitter.blit(this.page, source, rect.x + draw.dx, rect.y + draw.dy, draw.dw, draw.dh, srcFrame);
    } else {
      this.blitter.blit(this.page, source, rect.x, rect.y, width, height);
    }
    this.regionCount++;
    return new TexFrame(this.page.textures[0], rect.x, rect.y, width, height);
  }

  /** CO-PACK (shared atlas): allocate ONE `2·quadN × 2·quadN` frame and blit the four maps into its
   *  quadrants — albedo TL, normal TR, surface BL, layers BR (`sources` in that order; a null quadrant
   *  stays the page's cleared transparency). A prim references the returned 2N frame; each map is a
   *  fixed `quadN` offset away, so all four share one page at every lod (no per-map page, no offset drift).
   *  Optional per-quadrant `draws` apply the sprite-scale re-centre (rare); null = a straight quadrant blit. */
  addCoPacked(sources: Array<Texture | null>, quadN: number, padding: number, draws?: Array<AtlasDraw | null>): TexFrame | null {
    const full = quadN * 2;
    const rect = this.packer.insert(full + padding, full + padding);
    if (!rect) return null;
    const q = [[0, 0], [1, 0], [0, 1], [1, 1]]; // albedo, normal, surface, layers
    for (let i = 0; i < 4; i++) {
      const src = sources[i];
      if (!src) continue;
      const ox = rect.x + q[i][0] * quadN, oy = rect.y + q[i][1] * quadN;
      const draw = draws?.[i];
      if (draw) {
        const srcFrame = new TexFrame(src, draw.sx, draw.sy, draw.sw, draw.sh);
        this.blitter.blit(this.page, src, ox + draw.dx, oy + draw.dy, draw.dw, draw.dh, srcFrame);
      } else {
        this.blitter.blit(this.page, src, ox, oy, quadN, quadN);
      }
    }
    this.regionCount++;
    return new TexFrame(this.page.textures[0], rect.x, rect.y, full, full);
  }

  destroy(): void {
    this.page.destroy();
  }
}
