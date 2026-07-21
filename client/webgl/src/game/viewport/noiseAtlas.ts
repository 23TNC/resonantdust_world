//! Noise atlas — STUB for W4d. The pixijs version generates a tiling multi-field noise texture
//! the material bake samples for per-instance hue/chroma variation. The geo tier bakes flat
//! (no layers → the noise is never sampled), so this returns null until the material path is
//! wired with the real texture atlas. Re-port to build an engine `Texture` from generated noise
//! when material variation returns.

import type { Texture } from "../../gl";

export function makeNoiseAtlas(): Texture | null {
  return null;
}
