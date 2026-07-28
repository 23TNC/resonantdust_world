//! The material system's NOISE ATLAS — a small tiling texture whose rows are the noise
//! FIELDS (`NOISE_FIELDS` order) the material bake samples. Each field's R and G channels
//! carry two DECORRELATED tileable value-noise fields (one drives hue jitter, the other
//! chroma), so a single sample gives both. Generated procedurally at construction — no asset
//! fetch, no server dependency; `bin/lib/noise_fields.py` can later bake richer fields into
//! the same layout. Ported from pixijs (material-system P0 — the W4d stub returned null,
//! which silently zeroed EVERY material's variation).
//!
//! Character per field is set by frequency + anisotropy: `mottle` = mid-frequency isotropic
//! blobs; `strand` = fine vertically-stretched streaks; `needle` (material-system F4) =
//! short high-frequency directional dashes. All tile seamlessly (wrapping integer lattice)
//! so the shader's `fract()` sampling has no seam.

import { Texture } from "../../gl";
import { NOISE_FIELDS, type NoiseFieldName } from "./material";

/** Edge length (px) of one field's square tile — also the atlas width. Power of two. */
const FIELD_PX = 128;

/** A tiny deterministic PRNG (mulberry32) so the atlas is identical every run — a cached
 *  world bake wants stable inputs, never `Math.random`. */
function mulberry32(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a |= 0;
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** A tileable value-noise sampler over a wrapping `freqX×freqY` integer lattice; smaller
 *  lattice on one axis stretches the pattern (anisotropy). Smoothstep interpolation. */
function tileableNoise(rng: () => number, freqX: number, freqY: number): (x: number, y: number) => number {
  const grid = new Float32Array(freqX * freqY);
  for (let i = 0; i < grid.length; i++) grid[i] = rng();
  const at = (ix: number, iy: number): number =>
    grid[(((iy % freqY) + freqY) % freqY) * freqX + (((ix % freqX) + freqX) % freqX)];
  const smooth = (t: number): number => t * t * (3 - 2 * t);
  return (x, y) => {
    const fx = x * freqX;
    const fy = y * freqY;
    const x0 = Math.floor(fx);
    const y0 = Math.floor(fy);
    const tx = smooth(fx - x0);
    const ty = smooth(fy - y0);
    const a = at(x0, y0);
    const b = at(x0 + 1, y0);
    const c = at(x0, y0 + 1);
    const d = at(x0 + 1, y0 + 1);
    return a + (b - a) * tx + (c - a) * ty + (a - b - c + d) * tx * ty;
  };
}

/** Two-octave fbm over a tileable base — more texture than one octave, still seamless. */
function fbm(rng: () => number, freqX: number, freqY: number): (x: number, y: number) => number {
  const o1 = tileableNoise(rng, freqX, freqY);
  const o2 = tileableNoise(rng, freqX * 2, freqY * 2);
  return (x, y) => o1(x, y) * 0.67 + o2(x, y) * 0.33;
}

/** The (freqX, freqY) lattice per field — its spatial character. */
function latticeFor(field: NoiseFieldName): [number, number] {
  switch (field) {
    case "strand":
      return [4, 24]; // fine + vertically stretched → streaks
    case "mottle":
    default:
      return [6, 6]; // mid-frequency isotropic blobs
  }
}

/** Build the noise atlas: one `FIELD_PX`-tall row per `NOISE_FIELDS` entry, R/G = two
 *  decorrelated tileable fields of that entry's character. LINEAR-filtered (rgba8unorm) —
 *  the fields are smooth and the bake samples at arbitrary UV. */
export function makeNoiseAtlas(gl: WebGL2RenderingContext): Texture {
  const rows = NOISE_FIELDS.length;
  const data = new Uint8Array(FIELD_PX * FIELD_PX * rows * 4);
  NOISE_FIELDS.forEach((field, row) => {
    const [fx, fy] = latticeFor(field);
    // Distinct seeds per channel per field → R and G are independent.
    const nHue = fbm(mulberry32(0x9e37 + row * 131), fx, fy);
    const nChroma = fbm(mulberry32(0x1b56 + row * 977), fx, fy);
    for (let y = 0; y < FIELD_PX; y++) {
      for (let x = 0; x < FIELD_PX; x++) {
        const u = x / FIELD_PX;
        const v = y / FIELD_PX;
        const p = ((row * FIELD_PX + y) * FIELD_PX + x) * 4;
        data[p] = Math.round(nHue(u, v) * 255); // R → hue field
        data[p + 1] = Math.round(nChroma(u, v) * 255); // G → chroma field
        data[p + 2] = 0;
        data[p + 3] = 255;
      }
    }
  });
  return new Texture(gl, { width: FIELD_PX, height: FIELD_PX * rows, format: "rgba8unorm", nearest: false, data });
}
