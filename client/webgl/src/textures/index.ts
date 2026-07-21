//! Texture subsystem barrel. W3 login-boot subset: the resolver (stub) + the
//! renderer-agnostic URL/LOD helpers + the preview-cache persistence hint. The
//! atlas pools (`TextureAtlas`/`LodPool`/`MaxRectsPacker`) and the real resolver
//! return with the render layer in [webgl-engine W4](../../../docs/work/webgl-engine/todo.md).

export { TextureResolver } from "./TextureResolver";
export type { LodStats } from "./TextureResolver";
export { texturesRoot, realUrl, previewUrl, lodUrl, LOD_SIZES, pickLodForSize } from "./lod";
export { persistStorage } from "./previewCache";
