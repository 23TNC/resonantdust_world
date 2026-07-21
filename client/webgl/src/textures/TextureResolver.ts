//! TextureResolver — STUB for the W3 login boot. The real resolver owns the
//! three-tier prim resolution (master → preview → geo) over per-LOD MaxRects atlas
//! pools, baking sub-textures with the renderer — all GPU work that rides the
//! render layer being ported in [webgl-engine W4](../../../docs/work/webgl-engine/todo.md).
//! Login needs no textures, so this satisfies the boot surface (`setRoot` +
//! `lodStats` for the HUD) and no-ops the resolution API. Replace wholesale in W4
//! with the atlas-backed resolver built on the engine `Texture`/`RenderTarget`.

import type { Renderer } from "../gl";

/** LOD-pool occupancy for the debug HUD. Mirrors the real resolver's shape so the
 *  panel binds unchanged; the stub reports an empty set. */
export interface LodStats {
  /** Atlas pages summed across every LOD pool. */
  pages: number;
  /** Packed-texture count per pow2 LOD size (empty until W4). */
  counts: ReadonlyMap<number, number>;
  /** Preview (floor) tier module size in px. */
  previewSize: number;
  /** Packed preview modules. */
  previewCount: number;
}

const EMPTY_STATS: LodStats = { pages: 0, counts: new Map(), previewSize: 0, previewCount: 0 };

export class TextureResolver {
  private root: string;

  // Renderer is nullable in W3: the real resolver shares the viewport's GL
  // context, which doesn't exist until W4. Login needs no textures.
  constructor(_renderer: Renderer | null, texturesRoot: string) {
    this.root = texturesRoot;
  }

  /** Repoint at the world server's `/textures` base (recorded; no fetch in W3). */
  setRoot(root: string): void {
    this.root = root;
  }

  /** The `/textures` base this resolver would fetch from. */
  get texturesRoot(): string {
    return this.root;
  }

  /** HUD occupancy — empty until the atlas pools return (W4). */
  lodStats(): LodStats {
    return EMPTY_STATS;
  }

  /** Target LOD for the current zoom. No-op in the stub. */
  setTargetLod(_lod: number): void {}

  /** Resolve a named prim to a texture. Returns null in the stub (no consumers
   *  in the login path; the viewport that calls this returns in W4). */
  resolve(_name: string): null {
    return null;
  }

  /** Subscribe to in-place texture upgrades. No-op in the stub. */
  onLoad(_fn: () => void): void {}
}
