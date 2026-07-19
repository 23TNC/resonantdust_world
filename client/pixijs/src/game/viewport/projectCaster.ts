//! `projectCaster` (lighting P3/P4) — the shared billboard-silhouette SHEAR, ported from the old
//! game's `RectComposite.projectCaster`. Projects one caster's earcut silhouette (an
//! [`Outline`](../../textures/OutlineCache)) through a point light onto the ground: each silhouette
//! vertex slides AWAY from the light by a distance that grows with its height up the sprite
//! (`hUp/(lightZ−hUp)·dist`), and the earcut triangles are emitted (flat x,y pairs) into a vertex
//! buffer for the flat-fill scatter shader.
//!
//! Used two ways with the SAME math: **cold** shadows bake it once into the `cold_lightmap` (world
//! space, `pan = 0`); **dynamic** shadows run it per frame into the scatter lanes (`+pan`). Constants
//! live in [`scatterShader`](./scatterShader); they were tuned to the old coordinate space, so expect
//! a browser re-tune (`/showRT` the shadow tile).

import type { Outline } from "../../textures/OutlineCache";
import {
  SHADOW_DBL_CAP,
  SHADOW_MAX_LEN,
  SHADOW_MIN_Z,
  SHADOW_NORTH_STRETCH,
  shadowHeightScale,
} from "./scatterShader";

/** A shadow caster in WORLD px (`+pan` applied at projection). `footNY` = the silhouette's lowest
 *  contour point (normalized 0..1), the ground line; `feetX` its centre; `left`/`w` the sprite box. */
export interface Caster {
  feetX: number;
  groundY: number;
  footNY: number;
  h: number;
  w: number;
  left: number;
}

/** Reused scratch for one polygon's projected vertices (no per-caster allocation). */
const proj: number[] = [];

/** Project `caster`'s `outline` through the light at `(Lx, Ly, Lz)`, writing the sheared earcut
 *  triangles (flat x,y pairs) into `out` from vertex `vStart`; returns the next free vertex. `pan`
 *  offsets world → the target space (0 for the world-space cold bake). Stops at the buffer cap. */
export function projectCaster(
  out: Float32Array,
  vStart: number,
  caster: Caster,
  outline: Outline,
  Lx: number,
  Ly: number,
  Lz: number,
  panX: number,
  panY: number,
): number {
  const baseX = caster.feetX + panX;
  const baseY = caster.groundY + panY;
  const toLx = Lx - baseX;
  const toLy = Ly - baseY;
  const dBL = Math.hypot(toLx, toLy) || 1;
  const dirx = -toLx / dBL; // away from the light (one direction for the whole caster)
  const diry = -toLy / dBL;
  const dBLc = Math.min(dBL, SHADOW_DBL_CAP);
  const Lproj = Math.max(Lz, SHADOW_MIN_Z); // a low light still casts sane shadows (shadows only)
  const occScale = shadowHeightScale(caster.h);
  const hCap = Lproj * 0.95; // guards Lz − hUp → 0 only

  // Bound the WHOLE shadow: uniformly scale so its TIP (the projection of the silhouette's highest
  // point, smallest ny) lands at SHADOW_MAX_LEN — keeps the tip pointed (vs per-vertex clamping,
  // which slices it flat). Find the min ny (highest silhouette point) across all polygons.
  let minNy = 1.0;
  for (const pg of outline.polygons) {
    for (const [, ny] of pg.contour) if (ny < minNy) minNy = ny;
    for (const hole of pg.holes) for (const [, ny] of hole) if (ny < minNy) minNy = ny;
  }
  let topHUp = Math.max(caster.footNY - minNy, 0) * caster.h * occScale;
  if (topHUp > hCap) topHUp = hCap;
  const tipD = (topHUp / (Lproj - topHUp)) * dBLc;
  const scale = tipD > SHADOW_MAX_LEN ? SHADOW_MAX_LEN / tipD : 1.0;

  let v = vStart;
  for (const pg of outline.polygons) {
    // The flattened vertex list the triangle indices reference: contour ++ holes[0] ++ … .
    const verts: [number, number][] = pg.contour.slice();
    for (const hole of pg.holes) for (const p of hole) verts.push(p);
    proj.length = 0;
    for (const [nx, ny] of verts) {
      let hUp = Math.max(caster.footNY - ny, 0) * caster.h * occScale;
      if (hUp > hCap) hUp = hCap;
      const d = (hUp / (Lproj - hUp)) * dBLc * scale; // shadow length (uniform-scaled tip)
      let oy = diry * d;
      if (oy < 0) oy *= SHADOW_NORTH_STRETCH; // stretch only the north-going length
      // SHEAR: width stays horizontal (nx·w); the height shears toward the away-from-light dir.
      proj.push(caster.left + nx * caster.w + panX + dirx * d, baseY + oy);
    }
    const tris = pg.triangles;
    for (let t = 0; t + 2 < tris.length; t += 3) {
      if (v + 3 > out.length / 2) return v; // buffer full — caller doubles + retries
      for (let k = 0; k < 3; k++) {
        const idx = tris[t + k] * 2;
        out[v * 2] = proj[idx];
        out[v * 2 + 1] = proj[idx + 1];
        v++;
      }
    }
  }
  return v;
}
