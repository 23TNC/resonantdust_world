//! Texture subsystem: per-LOD MaxRects atlas pools that hand out batched
//! sub-textures, wrapped by the N-LOD resolver (geo → low LOD → target LOD) that
//! upgrades a prim's texture in place as the zoom's target LOD streams in.
//! Construct `TextureResolver` with the renderer at bootstrap; `resolver.white` is
//! the fill primitive (baked into the preview pool) ready immediately after.

export { TextureAtlas } from "./TextureAtlas";
export { MaxRectsPacker } from "./MaxRectsPacker";
export type { PackedRect } from "./MaxRectsPacker";
export { TextureResolver } from "./TextureResolver";
export type { LodStats } from "./TextureResolver";
export { LodPool } from "./LodPool";
export { texturesRoot, realUrl, previewUrl, lodUrl, LOD_SIZES, pickLodForSize } from "./lod";
export { persistStorage } from "./previewCache";
