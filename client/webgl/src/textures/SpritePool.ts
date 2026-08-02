//! The SPRITE POOL (one-resolution, 2026-08-02): a small set of {@link TextureAtlas} pages
//! holding every stem's ONE co-packed frame, keyed by stem. The per-LOD-size pool split died
//! with the atlas ladder — a stem packs once, at its manifest max, and the pow2-square LAW
//! (F3) is enforced here by REFUSING violating frames.
//!
//! Packing: a source is drawn into the first page with room, spilling to a fresh page when full, and
//! the returned {@link TexFrame} references that page's texture so all of a size's textures batch in
//! one draw. Only Pixi type swapped vs the original: `Texture`→`TexFrame`, `Renderer`→engine, + the
//! shared {@link Blitter}.

import type { TexFrame, Texture, Renderer, Blitter } from "../gl";
import { TextureAtlas, type AtlasDraw } from "./TextureAtlas";

/** Gutter around each packed texture. 0 is safe with nearest sampling + integer frames. */
const PADDING = 0;

export class SpritePool {
  private readonly renderer: Renderer;
  private readonly blitter: Blitter;
  private readonly pageSize: number;
  private readonly atlases: TextureAtlas[] = [];
  /** stem → its packed sub-frame at this pool's LOD size. */
  private readonly byStem = new Map<string, TexFrame>();

  /** `pageSize` sizes each atlas page — big enough to hold many of this LOD before spilling. */
  constructor(renderer: Renderer, blitter: Blitter, pageSize: number) {
    this.renderer = renderer;
    this.blitter = blitter;
    this.pageSize = pageSize;
    this.newAtlas();
  }

  has(stem: string): boolean {
    return this.byStem.has(stem);
  }
  get(stem: string): TexFrame | null {
    return this.byStem.get(stem) ?? null;
  }

  /** F4: the GRAPHICS twin of a frame's (data-page) source texture — the resolver re-sources
   *  albedo/normal quadrants through this. Null for a texture no page of ours owns. */
  graphicsTwin(source: Texture): Texture | null {
    for (const atlas of this.atlases) if (atlas.dataTex === source) return atlas.graphicsTex;
    return null;
  }

  /** CO-PACK a stem's four maps into ONE `2·quadN × 2·quadN` frame (quadrants: albedo TL, normal TR,
   *  surface BL, layers BR). Spills to a fresh page when full. Returns the 2N frame, or null if it can't
   *  fit a whole page. The def-grid invariant holds: `2·quadN` is pow2-square ≥16 and lands on the 16-px grid. */
  addCoPacked(stem: string, sources: Array<Texture | null>, quadN: number, draws?: Array<AtlasDraw | null>): TexFrame | null {
    const full = quadN * 2;
    // one-resolution F3: the LAW — refuse, never warn past (see add()).
    if ((full & (full - 1)) !== 0 || full < 16) {
      console.error(`[sprite-pool] ${stem}: co-pack frame ${full}×${full} violates the pow2-square ≥16 LAW — REFUSED (F3, 2026-08-02-one-resolution)`);
      return null;
    }
    // F3's second face: a WHOLE-BLIT source (no draw rect) must BE quadN² — brick/wall shipped a
    // 320-px normal beside 512-px siblings and the quadrant landed misregistered, silently. A
    // subframe draw carries its own source rect and may come from any size; a bare source may not.
    for (let i = 0; i < sources.length; i++) {
      const s = sources[i];
      if (s && !draws?.[i] && (s.width !== quadN || s.height !== quadN)) {
        console.error(`[sprite-pool] ${stem}: co-pack source ${i} is ${s.width}×${s.height}, quadrant is ${quadN} — REFUSED (F3: mixed-size map set)`);
        return null;
      }
    }
    let packed: TexFrame | null = null;
    for (const atlas of this.atlases) {
      packed = atlas.addCoPacked(sources, quadN, PADDING, draws);
      if (packed) break;
    }
    if (!packed) {
      if (full + PADDING > this.pageSize) return null;
      packed = this.newAtlas().addCoPacked(sources, quadN, PADDING, draws);
    }
    if (packed) {
      if (packed.x % 16 !== 0 || packed.y % 16 !== 0)
        console.warn(`[sprite-pool] ${stem}: co-pack frame off the 16-px grid (${packed.x},${packed.y})`);
      this.byStem.set(stem, packed);
    }
    return packed;
  }

  get pageCount(): number {
    return this.atlases.length;
  }
  get count(): number {
    return this.byStem.size;
  }

  private newAtlas(): TextureAtlas {
    const atlas = new TextureAtlas(this.renderer, this.blitter, this.pageSize);
    this.atlases.push(atlas);
    return atlas;
  }
}
