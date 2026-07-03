//! A LOD pool: a small set of {@link TextureAtlas} pages holding one LOD size's
//! textures, keyed by stem. A SEPARATE pool per LOD — a stem's 32px and 128px
//! textures live in different pools, so a size's pages pack uniformly (equal-size
//! rects → near-zero fragmentation) and don't compete. The built-in white fill is
//! packed into the preview-size pool, sharing its page rather than owning one.
//!
//! Packing: a source is drawn into the first page with room, spilling to a fresh
//! page when full, and the returned sub-texture frames onto that page's single
//! source so all of a size's textures batch in one draw.

import { Texture, type Renderer } from "pixi.js";
import { TextureAtlas } from "./TextureAtlas";

/** Gutter around each packed texture. 0 is safe with nearest sampling + integer
 *  frames (matches the master pool). */
const PADDING = 0;

export class LodPool {
  private readonly renderer: Renderer;
  private readonly pageSize: number;
  private readonly atlases: TextureAtlas[] = [];
  /** stem → its packed sub-texture at this pool's LOD size. */
  private readonly byStem = new Map<string, Texture>();

  /** `pageSize` sizes each atlas page — big enough to hold many of this LOD before
   *  spilling (1024 for the small buckets, 2048 for the large). */
  constructor(renderer: Renderer, pageSize: number) {
    this.renderer = renderer;
    this.pageSize = pageSize;
    this.newAtlas();
  }

  /** Whether `stem` is already packed. */
  has(stem: string): boolean {
    return this.byStem.has(stem);
  }

  /** `stem`'s packed texture, or null if it hasn't been added yet. */
  get(stem: string): Texture | null {
    return this.byStem.get(stem) ?? null;
  }

  /** Pack `source` (at its decoded `width × height`) as `stem`'s texture. Spills to
   *  a fresh page when the current ones are full. Returns the framed sub-texture, or
   *  null if it can't fit a whole page. */
  add(stem: string, source: Texture, width: number, height: number): Texture | null {
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

  /** Page count / packed-texture count, for the debug HUD. */
  get pageCount(): number {
    return this.atlases.length;
  }
  get count(): number {
    return this.byStem.size;
  }

  private newAtlas(): TextureAtlas {
    const atlas = new TextureAtlas(this.renderer, this.pageSize);
    this.atlases.push(atlas);
    return atlas;
  }
}
