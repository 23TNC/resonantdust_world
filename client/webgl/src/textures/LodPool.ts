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
import { TextureAtlas } from "./TextureAtlas";

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

  /** Pack `source` (at its decoded `width × height`) as `stem`'s texture. Spills to a fresh page when
   *  the current ones are full. Returns the framed sub-texture, or null if it can't fit a whole page. */
  add(stem: string, source: Texture, width: number, height: number): TexFrame | null {
    for (const atlas of this.atlases) {
      const packed = atlas.add(source, width, height, PADDING);
      if (packed) {
        this.byStem.set(stem, packed);
        return packed;
      }
    }
    if (width + PADDING > this.pageSize || height + PADDING > this.pageSize) return null;
    const packed = this.newAtlas().add(source, width, height, PADDING);
    if (packed) this.byStem.set(stem, packed);
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
