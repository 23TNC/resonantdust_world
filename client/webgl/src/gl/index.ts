//! The engine core (webgl-engine W2) — the bespoke WebGL2 renderer that replaces Pixi's backend, with
//! integer textures / MRT / vertex-texture-fetch first-class. `Program` (raw ES 3.00), `Geometry` (VAO +
//! instancing + integer attrs), `Texture` (unorm/float/integer), `RenderTarget` (FBO + MRT + integer/float
//! clears + always `drawBuffers`), `Renderer` (context + state + the draw primitive).

export { Program } from "./program";
export { Geometry, type AttribSpec } from "./geometry";
export { Texture, type TexFormat, type TextureOptions } from "./texture";
export { RenderTarget, type RenderTargetOptions } from "./renderTarget";
export { Renderer, type BlendMode, type DrawOptions } from "./renderer";
