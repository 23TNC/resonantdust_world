//! Texture subsystem: a MaxRects atlas pool that hands out batched sub-textures.
//! Construct `TextureManager` with the renderer at bootstrap; `manager.white`
//! is the 32×32 fill primitive ready immediately after.

export { TextureManager, TEXEL, WHITE } from "./TextureManager";
export type { AtlasStats } from "./TextureManager";
export { TextureAtlas } from "./TextureAtlas";
export { MaxRectsPacker } from "./MaxRectsPacker";
export type { PackedRect } from "./MaxRectsPacker";
