//! One GPU-backed atlas page: a square `RenderTexture` whose space is handed out
//! by a `MaxRectsPacker`. Adding a texture packs a rect, composites the source
//! into that rect, and returns a `Texture` framed onto this page's source — so
//! every sub-texture shares one `TextureSource` and batches in a single draw.

import {
  Container,
  Rectangle,
  RenderTexture,
  Sprite,
  Texture,
  type Renderer,
} from "pixi.js";
import { MaxRectsPacker } from "./MaxRectsPacker";

export class TextureAtlas {
  /** The page's backing render target. Sub-textures frame onto its `.source`. */
  readonly renderTexture: RenderTexture;

  private readonly renderer: Renderer;
  private readonly packer: MaxRectsPacker;
  private regionCount = 0;

  constructor(renderer: Renderer, size: number) {
    this.renderer = renderer;
    // resolution 1 keeps frame math in raw pixels; nearest avoids edge bleed
    // between packed neighbours (no mipmaps, integer frames → exact sampling).
    this.renderTexture = RenderTexture.create({
      width: size,
      height: size,
      resolution: 1,
      antialias: false,
      scaleMode: "nearest",
    });
    this.packer = new MaxRectsPacker(size, size);
    // Initialise the whole page to transparent so any not-yet-packed area never
    // shows uninitialised GPU garbage; every later add composites with clear:false.
    this.renderer.render({
      container: new Container(),
      target: this.renderTexture,
      clear: true,
      clearColor: [0, 0, 0, 0],
    });
  }

  /** Number of textures packed into this page. */
  get count(): number {
    return this.regionCount;
  }

  /** Fraction of the page's area handed out (0–1). */
  get occupancy(): number {
    return this.packer.occupancy;
  }

  /** Pack `source` (scaled to `width × height`) into a free rect, drawing it
   *  into the page. `padding` reserves a right/bottom gutter to keep neighbours
   *  from sampling into each other. Returns the framed sub-texture, or `null`
   *  if the page is full. */
  add(source: Texture, width: number, height: number, padding: number): Texture | null {
    const rect = this.packer.insert(width + padding, height + padding);
    if (!rect) return null;

    const sprite = new Sprite(source);
    sprite.position.set(rect.x, rect.y);
    sprite.width = width;
    sprite.height = height;
    // A straight-alpha source (a weight map like `packed`) must be copied VERBATIM: the
    // default premultiplied blend would multiply its RGB by its alpha and zero the weights
    // wherever alpha is 0. Rects never overlap, so an overwrite blit is safe for all sources.
    if (source.source.alphaMode === "no-premultiply-alpha") sprite.blendMode = "none";
    this.renderer.render({ container: sprite, target: this.renderTexture, clear: false });
    sprite.destroy(); // drops the sprite, not the shared source texture

    this.regionCount++;
    return new Texture({
      source: this.renderTexture.source,
      frame: new Rectangle(rect.x, rect.y, width, height),
    });
  }
}
