//! The shadow buffer (lighting-rework P4) — 3 px per UNIT, ping-ponged.
//!
//! **Per UNIT, not per tile** ([F4](forks.md#f4)). The design's prose says "three px per TILE", but
//! the algorithm reads *adjacent units* (±x, ±y) and runs `UNITS_PER_TILE²` = 256 fragments per tile.
//! Per-tile would give all 256 fragments one shared record and destroy the resolution the whole
//! design is built on. It is a **256×** memory difference — ~6 MB per buffer, not ~24 KB — which is
//! why it is stated here rather than left to be inferred.
//!
//! ```
//! px 0   the caster occluding each of the 8 lights for presence[0] (the ground)
//! px 1   4 x (caster, receiver) pairs — lights 0..3
//! px 2   4 x (caster, receiver) pairs — lights 4..7
//! ```
//!
//! **The slot split is `l >= 4`, not `l > 4`.** The design's `l > 4` sends lights 0..4 to px 1 with
//! `ll = l*2`, so light 4 lands at `ll = 8` — one past the end of a px that holds 8 `u16` slots — and
//! px 2's first two slots are never used at all. One light in eight silently never shadowing is
//! exactly the kind of defect that reads as "the shadows look wrong" for a week
//! ([I1](issues.md#i1)).
//!
//! **Ping-pong** ([F4](forks.md#f4)): the adjacency step reads neighbouring units' shadow while those
//! fragments are writing theirs, and a fragment shader cannot read the attachment it writes. Read
//! last frame, write this frame. Staleness is safe *by construction* — adjacency is an accelerator
//! and a stale miss falls through to the corridor walk, which is authoritative — but the buffers must
//! be **cleared to 0**, because 0 is the sentinel meaning "no caster, take the slow path" whereas
//! garbage would name a real prim.

import { RenderTarget, type Texture } from "../../gl";
import { UNITS_PER_TILE, SLOTS_X, SLOTS_Y } from "./squareMath";

/** px per unit: ground casters, then two px of (caster, receiver) pairs. */
export const SHADOW_PX_PER_UNIT = 3;
/** Lights per unit — matches `TILE_SLOTS`, since a shadow slot mirrors a light slot. */
export const SHADOW_LIGHTS = 8;

const UNITS_X = SLOTS_X * UNITS_PER_TILE;      // 512
const UNITS_Y = SLOTS_Y * UNITS_PER_TILE;      // 256

/** Which px and which `u16` slot hold light `l`'s (caster, receiver) pair.
 *
 *  THE correction from [I1](issues.md#i1) — `l >= 4`, so lights 0..3 fill px 1 slots 0,2,4,6 and
 *  lights 4..7 fill px 2 slots 0,2,4,6. Every light addresses a distinct slot and none overflows. */
export function pairSlot(light: number): { px: number; slot: number } {
  return light >= 4
    ? { px: 2, slot: (light - 4) * 2 }
    : { px: 1, slot: light * 2 };
}

export class ShadowBuffer {
  /** Read from `prev`, write to `cur`, then {@link swap}. */
  private a: RenderTarget;
  private b: RenderTarget;

  constructor(private readonly gl: WebGL2RenderingContext) {
    this.a = ShadowBuffer.alloc(gl);
    this.b = ShadowBuffer.alloc(gl);
    this.clear();
  }

  private static alloc(gl: WebGL2RenderingContext): RenderTarget {
    return new RenderTarget(gl, {
      width: UNITS_X * SHADOW_PX_PER_UNIT, height: UNITS_Y, formats: ["rgba32uint"],
    });
  }

  /** Both buffers to 0 — the "no caster" sentinel, so the first frame takes the slow path rather
   *  than trusting whatever the allocation left behind. */
  clear(): void {
    const zero = new Uint32Array([0, 0, 0, 0]);
    for (const rt of [this.a, this.b]) {
      rt.bind();
      this.gl.clearBufferuiv(this.gl.COLOR, 0, zero);
    }
    this.gl.bindFramebuffer(this.gl.FRAMEBUFFER, null);
  }

  swap(): void { const t = this.a; this.a = this.b; this.b = t; }

  get cur(): RenderTarget { return this.a; }
  get prev(): Texture { return this.b.textures[0]; }

  get bytes(): number {
    return UNITS_X * SHADOW_PX_PER_UNIT * UNITS_Y * 16 * 2;   // x2: ping-pong
  }

  get dims(): { unitsX: number; unitsY: number; w: number; h: number; pxPerUnit: number } {
    return {
      unitsX: UNITS_X, unitsY: UNITS_Y,
      w: UNITS_X * SHADOW_PX_PER_UNIT, h: UNITS_Y, pxPerUnit: SHADOW_PX_PER_UNIT,
    };
  }

  destroy(): void { this.a.destroy(); this.b.destroy(); }
}
