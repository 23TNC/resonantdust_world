//! One GPU-backed atlas page (webgl port) — SPLIT into TWIN textures (one-resolution F4, user):
//! ONE {@link MaxRectsPacker} hands out ONE `2N×2N` rect per co-packed stem, written to TWO
//! same-size page textures:
//!
//!   • the GRAPHICS page — albedo TL + normal TR + layers BR quadrants. Carries a capped MIP
//!     CHAIN and filtered sampling: blendable presentation maps. The normal's sole consumer
//!     renormalizes after decode, so filtering it is safe (unfiltered normals shimmer under
//!     minification); layers filters EXACTLY because the reconstruction is linear in the
//!     weights (out = residual + Σ w·tint, tints constant — filtering commutes; user's call).
//!   • the DATA page — the surface BL quadrant alone. NEAREST, NO mips: `silhouetteHit`
//!     thresholds `surface.b` — that read must see bytes nobody blended.
//!
//! Frame coordinates are IDENTICAL on both pages (one packer, one rect), so def addressing and
//! every shader's quadrant math are untouched — only which TEXTURE a map's frame points at
//! differs, and the resolver hands each map its page. The premultiply (colour vs straight-alpha
//! data maps) lives in the source `Texture`'s upload, not here.

import { RenderTarget, TexFrame, Texture, type Renderer, type Blitter } from "../gl";
import { MaxRectsPacker } from "./MaxRectsPacker";

/** A sub-rect composite (P5 ingest scale/clip/re-centre): draw the source's `(sx,sy,sw,sh)` px
 *  sub-rect into the frame's `(dx,dy,dw,dh)` px sub-rect; the rest of the frame stays the page's
 *  cleared transparency. */
export interface AtlasDraw {
  dx: number; dy: number; dw: number; dh: number;
  sx: number; sy: number; sw: number; sh: number;
}

/** Mip levels on the graphics page: 0..2 (4× minification — the deepest the partition reaches
 *  at zoom 0.25). The cap is the atlas-bleed bound: level k blends 2^k-px blocks, and frames
 *  have no gutter, so deeper levels would blend neighbouring stems. */
const GRAPHICS_MIP_LEVELS = 3;

export class TextureAtlas {
  /** The GRAPHICS page texture (albedo + normal) — mipped, filtered. Frames for those maps
   *  reference this. */
  readonly graphicsTex: Texture;
  /** The DATA page texture (surface + layers) — NEAREST, exact. Co-pack frames reference this
   *  (the silhouette consumers bind it), and the resolver re-sources graphics-map quadrants. */
  readonly dataTex: Texture;

  private readonly graphicsRT: RenderTarget;
  private readonly dataRT: RenderTarget;
  private readonly blitter: Blitter;
  private readonly packer: MaxRectsPacker;
  private regionCount = 0;

  constructor(renderer: Renderer, blitter: Blitter, size: number) {
    this.blitter = blitter;
    const gl = renderer.gl;
    // Graphics: LINEAR base (magnification) + trilinear mips (minification) — the "aa again" half.
    this.graphicsTex = new Texture(gl, { width: size, height: size, nearest: false, mipLevels: GRAPHICS_MIP_LEVELS });
    // Data: NEAREST + integer frames → exact sampling, no edge bleed between packed neighbours.
    this.dataTex = new Texture(gl, { width: size, height: size });
    this.graphicsRT = new RenderTarget(gl, { width: size, height: size, wrap: [this.graphicsTex] });
    this.dataRT = new RenderTarget(gl, { width: size, height: size, wrap: [this.dataTex] });
    this.packer = new MaxRectsPacker(size, size);
    // Initialise both pages transparent so a not-yet-packed area never shows GPU garbage;
    // every later add composites with overwrite (blend "none"), never clearing.
    this.graphicsRT.clear(0, 0, 0, 0);
    this.dataRT.clear(0, 0, 0, 0);
  }

  /** Number of stems packed into this page pair. */
  get count(): number {
    return this.regionCount;
  }
  /** Fraction of the page's area handed out (0–1). */
  get occupancy(): number {
    return this.packer.occupancy;
  }

  /** CO-PACK (split atlas): allocate ONE `2·quadN × 2·quadN` rect and blit the four maps into its
   *  quadrants across the TWIN pages — albedo TL + normal TR onto the GRAPHICS page, surface BL +
   *  layers BR onto the DATA page (`sources` in that order; a null quadrant stays the page's
   *  cleared transparency). The returned frame references the DATA page (the silhouette binding);
   *  graphics-map quadrants re-source via {@link graphicsTex}. Optional per-quadrant `draws`
   *  apply the subframe-ingest scale/clip; null = a straight quadrant blit. */
  addCoPacked(sources: Array<Texture | null>, quadN: number, padding: number, draws?: Array<AtlasDraw | null>): TexFrame | null {
    const full = quadN * 2;
    const rect = this.packer.insert(full + padding, full + padding);
    if (!rect) return null;
    const q = [[0, 0], [1, 0], [0, 1], [1, 1]]; // albedo, normal, surface, layers
    // F4: the split — surface alone is DATA (thresholded silhouette bytes). Layers is GRAPHICS
    // by the user's call: the reconstruction is LINEAR in the weights (out = residual + Σ w·tint,
    // tints constant), so linear filtering commutes with it exactly — channel-separated weights
    // filter as well as the albedo they modulate.
    const pageFor = [this.graphicsRT, this.graphicsRT, this.dataRT, this.graphicsRT];
    let touchedGraphics = false;
    for (let i = 0; i < 4; i++) {
      const src = sources[i];
      if (!src) continue;
      const ox = rect.x + q[i][0] * quadN, oy = rect.y + q[i][1] * quadN;
      const draw = draws?.[i];
      if (draw) {
        const srcFrame = new TexFrame(src, draw.sx, draw.sy, draw.sw, draw.sh);
        this.blitter.blit(pageFor[i], src, ox + draw.dx, oy + draw.dy, draw.dw, draw.dh, srcFrame);
      } else {
        this.blitter.blit(pageFor[i], src, ox, oy, quadN, quadN);
      }
      if (i < 2) touchedGraphics = true;
    }
    // The blit mutates level 0 only — rebuild the graphics mip chain (data has none).
    if (touchedGraphics) this.graphicsTex.regenerateMips();
    this.regionCount++;
    return new TexFrame(this.dataTex, rect.x, rect.y, full, full);
  }

  destroy(): void {
    this.graphicsRT.destroy();
    this.dataRT.destroy();
    // wrap: [] targets do NOT own their textures — destroy the twins explicitly.
    this.graphicsTex.destroy();
    this.dataTex.destroy();
  }
}
