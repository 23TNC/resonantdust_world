//! Texture subsystem barrel. W3 login-boot subset: the resolver (stub) + the
//! renderer-agnostic URL helpers + the preview-cache persistence hint. The
//! atlas pools (`TextureAtlas`/`SpritePool`/`MaxRectsPacker`) and the real resolver
//! return with the render layer in [webgl-engine W4](../../../docs/work/webgl-engine/todo.md).

export { TextureResolver } from "./TextureResolver";
export type { PoolStats, ResolvedTexture } from "./TextureResolver";
export { TextureAtlas } from "./TextureAtlas";
export { SpritePool } from "./SpritePool";
export { texturesRoot, texUrl, type TexMap } from "./urls";
export { persistStorage } from "./previewCache";
