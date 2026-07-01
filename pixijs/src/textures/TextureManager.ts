//! The texture manager — owns the pool of `TextureAtlas` pages and hands out
//! framed sub-textures. Rebuilt for rectangle textures (multiples of 32px)
//! packed via MaxRects, replacing the old power-of-two quadtree atlas.
//!
//! At construction it bakes the `WHITE` texture: a 32×32 solid-white region used
//! everywhere as the tint/fill primitive (rectangles, lines, panel backings).
//! Texture *loading* returns later; for now `add()` accepts any in-memory
//! `Texture` (e.g. `Texture.WHITE`) and the manager bootstraps with WHITE alone.

import { Texture, type Renderer } from "pixi.js";
import { TextureAtlas } from "./TextureAtlas";

/** Square page size, in pixels. 2048 is a safe WebGL floor and a multiple of
 *  32, so 32-aligned rects tile it without remainder. */
const ATLAS_SIZE = 2048;

/** The base grid unit — all game textures are multiples of this. */
export const TEXEL = 32;

/** Reserved name of the built-in white fill. A *special case*: it's baked from
 *  `Texture.WHITE` at construction and resolved locally by {@link TextureManager.get},
 *  so it never reaches out to R2 (which hasn't been re-implemented yet). */
export const WHITE = "white";

/** Gutter reserved around each packed rect. 0 is safe today (nearest sampling
 *  + integer frames don't bleed); bump it if linear filtering or mipmaps return. */
const PADDING = 0;

/** Atlas occupancy readout for the debug HUD. */
export interface AtlasStats {
  /** Number of atlas pages. */
  atlases: number;
  /** Total textures packed across all pages. */
  regions: number;
  /** Per-page fill fraction (0–1), one entry per atlas. */
  occupancy: number[];
}

export class TextureManager {
  private readonly renderer: Renderer;
  private readonly atlases: TextureAtlas[] = [];

  /** Textures resolved locally by name — never fetched from R2. Seeded with the
   *  built-in {@link WHITE}; future generated/baked textures register here too. */
  private readonly builtins = new Map<string, Texture>();

  /** The all-purpose 32×32 solid-white texture. Tint it for flat fills/lines. */
  readonly white: Texture;

  constructor(renderer: Renderer) {
    this.renderer = renderer;
    this.newAtlas();
    // WHITE is a special case: baked from the in-memory `Texture.WHITE`, with no
    // R2 round-trip. Registered under its reserved name so `get(WHITE)` resolves
    // it locally once name-based lookup is the norm.
    const white = this.add(Texture.WHITE, TEXEL, TEXEL);
    if (!white) throw new Error("TextureManager: failed to pack the WHITE texture");
    this.white = white;
    this.builtins.set(WHITE, white);
  }

  /** Resolve a texture by name. Built-ins (e.g. {@link WHITE}) resolve locally
   *  and never touch the network. Any other name will load from R2 once that
   *  path returns — until then it's unavailable (`null`), and the built-in
   *  short-circuit above guarantees WHITE never waits on it. */
  get(name: string): Texture | null {
    return this.builtins.get(name) ?? null;
  }

  /** Pack `source` (scaled to `width × height`) into the first atlas with room,
   *  spilling to a fresh page when none fit. Returns the framed sub-texture, or
   *  `null` if the request is larger than a whole atlas page. */
  add(source: Texture, width: number, height: number): Texture | null {
    for (const atlas of this.atlases) {
      const packed = atlas.add(source, width, height, PADDING);
      if (packed) return packed;
    }
    if (width + PADDING > ATLAS_SIZE || height + PADDING > ATLAS_SIZE) return null;
    return this.newAtlas().add(source, width, height, PADDING);
  }

  /** Snapshot of pool occupancy for the debug HUD. */
  getAtlasStats(): AtlasStats {
    return {
      atlases: this.atlases.length,
      regions: this.atlases.reduce((sum, a) => sum + a.count, 0),
      occupancy: this.atlases.map((a) => a.occupancy),
    };
  }

  private newAtlas(): TextureAtlas {
    const atlas = new TextureAtlas(this.renderer, ATLAS_SIZE);
    this.atlases.push(atlas);
    return atlas;
  }
}
