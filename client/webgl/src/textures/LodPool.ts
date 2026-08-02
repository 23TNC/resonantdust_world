//! A LOD pool (webgl port): a small set of {@link TextureAtlas} pages holding one LOD size's
//! textures, keyed by stem. A SEPARATE pool per LOD — a stem's 32px and 128px textures live in
//! different pools, so a size's pages pack uniformly (equal-size rects → near-zero fragmentation) and
//! don't compete. The built-in white fill is packed into the preview-size pool, sharing its page
//! rather than owning one.
//!
//! Packing: a source is drawn into the first page with room, spilling to a fresh page when full, and
//! the returned {@link TexFrame} references that page's texture so all of a size's textures batch in
//! one draw. Only Pixi type swapped vs the original: `Texture`→`TexFrame`, `Renderer`→engine, + the
//! shared {@link Blitter}.

import type { TexFrame, Texture, Renderer, Blitter } from "../gl";
import { TextureAtlas, type AtlasDraw } from "./TextureAtlas";

/** Gutter around each packed texture. 0 is safe with nearest sampling + integer frames. */
const PADDING = 0;

export class LodPool {
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

  /** Pack `source` as `stem`'s texture into a `width × height` frame. Spills to a fresh page when
   *  the current ones are full. `draw`, if given, composites the source into a SUB-rect of the frame
   *  from a source sub-rect (the P5 ingest scale/clip/re-centre; the rest of the frame stays the
   *  page-clear transparency). Returns the framed sub-texture, or null if it can't fit a whole page. */
  add(stem: string, source: Texture, width: number, height: number, draw?: AtlasDraw): TexFrame | null {
    // Def-grid invariants (def-frame-anchors P1): frames must be pow2 SQUARES ≥ 16px, placed on the
    // 16-px page grid — the shadow def addresses them by u4 lod exponent + u10 16-px-grid origin.
    // one-resolution F3: the LAW — a violating frame is REFUSED, not warned past. A warn is how a
    // non-pow2 master drifted in unseen; a refused pack is loud (the stem renders geo until fixed).
    if (width !== height || (width & (width - 1)) !== 0 || width < 16) {
      console.error(`[lod-pool] ${stem}: frame ${width}×${height} violates the pow2-square ≥16 LAW — REFUSED (F3, 2026-08-02-one-resolution)`);
      return null;
    }
    let packed: TexFrame | null = null;
    for (const atlas of this.atlases) {
      packed = atlas.add(source, width, height, PADDING, draw);
      if (packed) break;
    }
    if (!packed) {
      if (width + PADDING > this.pageSize || height + PADDING > this.pageSize) return null;
      packed = this.newAtlas().add(source, width, height, PADDING, draw);
    }
    if (packed) {
      if (packed.x % 16 !== 0 || packed.y % 16 !== 0)
        console.warn(`[lod-pool] ${stem}: frame placed off the 16-px grid (${packed.x},${packed.y})`);
      this.byStem.set(stem, packed);
    }
    return packed;
  }

  /** CO-PACK a stem's four maps into ONE `2·quadN × 2·quadN` frame (quadrants: albedo TL, normal TR,
   *  surface BL, layers BR). Spills to a fresh page when full. Returns the 2N frame, or null if it can't
   *  fit a whole page. The def-grid invariant holds: `2·quadN` is pow2-square ≥16 and lands on the 16-px grid. */
  addCoPacked(stem: string, sources: Array<Texture | null>, quadN: number, draws?: Array<AtlasDraw | null>): TexFrame | null {
    const full = quadN * 2;
    // one-resolution F3: the LAW — refuse, never warn past (see add()).
    if ((full & (full - 1)) !== 0 || full < 16) {
      console.error(`[lod-pool] ${stem}: co-pack frame ${full}×${full} violates the pow2-square ≥16 LAW — REFUSED (F3, 2026-08-02-one-resolution)`);
      return null;
    }
    // F3's second face: a WHOLE-BLIT source (no draw rect) must BE quadN² — brick/wall shipped a
    // 320-px normal beside 512-px siblings and the quadrant landed misregistered, silently. A
    // subframe draw carries its own source rect and may come from any size; a bare source may not.
    for (let i = 0; i < sources.length; i++) {
      const s = sources[i];
      if (s && !draws?.[i] && (s.width !== quadN || s.height !== quadN)) {
        console.error(`[lod-pool] ${stem}: co-pack source ${i} is ${s.width}×${s.height}, quadrant is ${quadN} — REFUSED (F3: mixed-size map set)`);
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
        console.warn(`[lod-pool] ${stem}: co-pack frame off the 16-px grid (${packed.x},${packed.y})`);
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
