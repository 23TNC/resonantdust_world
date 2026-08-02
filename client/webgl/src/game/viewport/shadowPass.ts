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

import { Program, Geometry, RenderTarget, type Renderer, type Texture } from "../../gl";
import { UNITS_PER_TILE, SLOTS_X, SLOTS_Y } from "./squareMath";
import { LIGHT_LANES_GLSL, OCCLUSION_GLSL } from "./records";


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

const FULLSCREEN_VERT = /* glsl */ `#version 300 es
in vec2 aPos;
void main() { gl_Position = vec4(aPos, 0.0, 1.0); }
`;

/** The gather. One fragment per unit-px; three tiers, cheapest first. */
const GATHER_FRAG = /* glsl */ `#version 300 es
precision highp float;
precision highp int;
uniform sampler2D uSurfaceAtlas;   // the shared co-packed page — the SILHOUETTE lives here
uniform sampler2D uSurfaceAtlas2;  // P3: the second page (defs carry their page in anchor.x)
uniform highp usampler2D uPrim;
uniform highp usampler2D uDef;
uniform highp usampler2D uLight;
uniform highp usampler2D uPresence;
uniform highp usampler2D uPrev;      // LAST frame's shadow -- ping-pong, never the one we write
uniform vec2 uWindowOrigin;
uniform int uUnitT;                  // P4: shadow texels per TILE (TEXTILE_UNIT >> level)
uniform int uCols;                   // P4: the slot torus modulus (window tiles)
uniform int uRows;
uniform int uDebugTier;              // 1 = emit which TIER answered, for the hit-rate histogram
uniform int uBrute;                  // 1 = EXHAUSTIVE search, the reference the walk must match
uniform int uDilateX;                // tiles of x-dilation: a card is WIDER than its tile
out uvec4 fragColor;

${LIGHT_LANES_GLSL}

const float UPT      = ${UNITS_PER_TILE}.0;
const int   TILE_DIM = 256;
const int   PX_PER_UNIT = ${SHADOW_PX_PER_UNIT};
const int   LIGHTS   = ${SHADOW_LIGHTS};
const uint  NONE     = 0u;

int pmod(int a, int m) { return ((a % m) + m) % m; }
uvec4 fetchPrim(uint i) { return texelFetch(uPrim, ivec2(int(i) & 1023, int(i) >> 10), 0); }
uvec4 fetchDef(uint block, uint rot) {
  uint px = block * 16u + rot;
  return texelFetch(uDef, ivec2(int(px) & 1023, int(px) >> 10), 0);
}
uint slotOf(uvec4 v, int i) {
  uint w = i < 2 ? v.x : (i < 4 ? v.y : (i < 6 ? v.z : v.w));
  return (i & 1) == 0 ? (w >> 16) : (w & 0xffffu);
}

${OCCLUSION_GLSL}

void main() {
  ivec2 fc = ivec2(gl_FragCoord.xy);
  int px   = fc.x % PX_PER_UNIT;             // which of the 3 px of this unit
  int ux   = fc.x / PX_PER_UNIT;
  int uy   = fc.y;

  // P4: the buffer rides the SLOT TORUS at uUnitT texels/tile — unwrap the fragment's residue to
  // its window tile (the composites' rule), then place the tested point at the TEXEL'S CENTRE in
  // world units (at level > 0 one texel spans several units; the centre is the representative).
  int tileX = int(uWindowOrigin.x) + pmod(ux / uUnitT - int(uWindowOrigin.x), uCols);
  int tileY = int(uWindowOrigin.y) + pmod(uy / uUnitT - int(uWindowOrigin.y), uRows);
  float upt = UPT / float(uUnitT);   // world units per shadow texel
  vec2 P = vec2(float(tileX) * UPT + (float(ux % uUnitT) + 0.5) * upt,
                float(tileY) * UPT + (float(uy % uUnitT) + 0.5) * upt);

  ivec2 tileTex = ivec2(pmod(tileX, TILE_DIM), pmod(tileY, TILE_DIM));
  uvec4 lights   = texelFetch(uLight,    tileTex, 0);
  uvec4 presence = texelFetch(uPresence, tileTex, 0);

  uvec4 prev = texelFetch(uPrev, fc, 0);
  uint out0 = 0u, out1 = 0u, out2 = 0u, out3 = 0u;
  uint tiers = 0u;

  for (int l = 0; l < LIGHTS; l++) {
    uint li = slotOf(lights, l);
    if (li == NONE) continue;
    uvec4 lrec = fetchPrim(li);
    if (((lrec.z >> 26) & 3u) == 0u) continue;               // F9: not an emitter any more
    vec2  L  = primPos(lrec);       // P5: fine-refined emitter position
    float Lz = primElevation(lrec);
    float reach = reachUnitsFromB(lrec.z);
    if (reach <= 0.0 || distance(P, L) >= reach) continue;   // F6 bounds the whole search

    uint found = NONE;
    uint tier  = 0u;

    // BRUTE reference: scan every tile in the reach box and every occupant of each. No incumbent,
    // no adjacency, no corridor -- the answer the three-tier path has to reproduce exactly.
    if (uBrute == 1) {
      int rt = int(ceil(reach / UPT));
      int ptx = int(floor(P.x / UPT)), pty = int(floor(P.y / UPT));
      for (int dy = -rt; dy <= rt; dy++) {
        for (int dx = -rt; dx <= rt; dx++) {
          if (dx * dx + dy * dy > rt * rt) continue;
          uvec4 occ = texelFetch(uPresence, ivec2(pmod(ptx + dx, TILE_DIM), pmod(pty + dy, TILE_DIM)), 0);
          for (int k = 1; k < 8; k++) {
            uint cand = slotOf(occ, k);
            if (cand != NONE && occludes(cand, L, Lz, P)) { found = cand; tier = 3u; break; }
          }
          if (found != NONE) break;
        }
        if (found != NONE) break;
      }
    }

    // TIER 1 -- the incumbent. Under a stored IDENTITY this is a complete answer, not a hint:
    // if last frame's caster still occludes, nothing else needs looking at.
    uint inc = slotOf(prev, l);
    if (uBrute == 0 && occludes(inc, L, Lz, P)) { found = inc; tier = 1u; }

    // TIER 2 -- the four adjacent units' incumbents. A shadow edge moves a unit at a time, so a
    // neighbour's caster is overwhelmingly the next one to be ours. Reads the PREV buffer, so a
    // stale value is safe: a miss just falls through.
    if (found == NONE && uBrute == 0) {
      for (int a = 0; a < 4; a++) {
        ivec2 o = a == 0 ? ivec2(-1, 0) : (a == 1 ? ivec2(1, 0) : (a == 2 ? ivec2(0, -1) : ivec2(0, 1)));
        // P4: toroidal neighbours — an edge texel's wrap lands on the WORLD-adjacent tile's texel
        // (the slot torus's defining property), so no bounds check exists to fail.
        int nx = pmod(ux + o.x, uCols * uUnitT), ny = pmod(uy + o.y, uRows * uUnitT);
        uint cand = slotOf(texelFetch(uPrev, ivec2(nx * PX_PER_UNIT + px, ny), 0), l);
        if (occludes(cand, L, Lz, P)) { found = cand; tier = 2u; break; }
      }
    }

    // TIER 3 -- the corridor, as a SUPERCOVER DDA (Amanatides-Woo).
    //
    // A point-march along the segment SKIPS tiles it clips diagonally, and every skipped tile is a
    // caster that silently never shadows. Measured: a naive march disagreed with an exhaustive
    // search on 1116 of 65536 words. Stepping boundary to boundary visits every tile the segment
    // touches, by construction, so the walk and the reference agree.
    if (found == NONE && uBrute == 0) {
      vec2 dir = P - L;
      vec2 tile0 = floor(L / UPT), tile1 = floor(P / UPT);
      ivec2 c = ivec2(tile0);
      ivec2 endT = ivec2(tile1);
      ivec2 stepD = ivec2(dir.x > 0.0 ? 1 : -1, dir.y > 0.0 ? 1 : -1);
      vec2 inv = vec2(abs(dir.x) < 1e-6 ? 1e18 : UPT / abs(dir.x),
                      abs(dir.y) < 1e-6 ? 1e18 : UPT / abs(dir.y));
      vec2 nextB = vec2(float(c.x + (stepD.x > 0 ? 1 : 0)) * UPT,
                        float(c.y + (stepD.y > 0 ? 1 : 0)) * UPT);
      vec2 tMax = vec2(abs(dir.x) < 1e-6 ? 1e18 : abs((nextB.x - L.x) / dir.x),
                       abs(dir.y) < 1e-6 ? 1e18 : abs((nextB.y - L.y) / dir.y));
      for (int s = 0; s < 96; s++) {
        // X-DILATION. A caster occludes when its CARD crosses the ray, and a card is wider than the
        // tile it registers in -- so a caster several tiles off the corridor in x still shadows. The
        // walk visits tiles; occlusion is about cards. Without this the walk disagreed with an
        // exhaustive search on ~1100 of 65536 words, and every disagreement is a shadow that is
        // simply absent. Only x needs it: the card is horizontal, so a ray crosses each ROW once.
        for (int dy2 = 0; dy2 <= uDilateX; dy2++) {          // P3: ns cards extend in +y from
          for (int dx = -uDilateX; dx <= uDilateX; dx++) {   // their base like ew cards in x
            uvec4 occ = texelFetch(uPresence, ivec2(pmod(c.x + dx, TILE_DIM), pmod(c.y + dy2, TILE_DIM)), 0);
            for (int k = 1; k < 8; k++) {
              uint cand = slotOf(occ, k);
              if (cand != NONE && occludes(cand, L, Lz, P)) { found = cand; tier = 3u; break; }
            }
            if (found != NONE) break;
          }
          if (found != NONE) break;
        }
        if (found != NONE) break;
        if (c == endT) break;
        if (tMax.x < tMax.y) { tMax.x += inv.x; c.x += stepD.x; }
        else                 { tMax.y += inv.y; c.y += stepD.y; }
      }
    }

    tiers = tiers | (tier << uint(l * 2));

    // px 0 holds the GROUND caster per light; px 1/2 hold (caster, receiver) pairs -- see pairSlot().
    if (px == 0) {
      if (l < 2)      out0 = out0 | (found << uint((1 - (l & 1)) * 16));
      else if (l < 4) out1 = out1 | (found << uint((1 - (l & 1)) * 16));
      else if (l < 6) out2 = out2 | (found << uint((1 - (l & 1)) * 16));
      else            out3 = out3 | (found << uint((1 - (l & 1)) * 16));
    } else {
      int base = px == 1 ? 0 : 4;                            // px1 -> lights 0..3, px2 -> 4..7
      if (l >= base && l < base + 4) {
        uint r = slotOf(presence, 1);                        // topmost non-tile receiver, if any
        int pair = (l - base) * 2;
        uint cw = (found << 16) | (r & 0xffffu);
        if (pair == 0) out0 = cw; else if (pair == 2) out1 = cw;
        else if (pair == 4) out2 = cw; else out3 = cw;
      }
    }
  }

  fragColor = uDebugTier == 1 ? uvec4(tiers, 0u, 0u, 0u) : uvec4(out0, out1, out2, out3);
}
`;

export class ShadowBuffer {
  /** Read from `prev`, write to `cur`, then {@link swap}. */
  private a: RenderTarget;
  private b: RenderTarget;
  private readonly prog: Program;
  private readonly quad: Geometry;

  constructor(private readonly gl: WebGL2RenderingContext) {
    this.a = ShadowBuffer.alloc(gl);
    this.b = ShadowBuffer.alloc(gl);
    this.prog = new Program(gl, FULLSCREEN_VERT, GATHER_FRAG, "shadow-gather");
    this.quad = new Geometry(gl, this.prog, {
      aPos: { data: new Float32Array([-1, -1, 1, -1, 1, 1, -1, 1]), size: 2 },
    }, new Uint32Array([0, 1, 2, 0, 2, 3]));
    this.clear();
  }

  /** One gather: read PREV, write CUR, swap. `debugTier` emits which tier answered instead of the
   *  caster, so the hit rates are measured rather than assumed. */
  gather(renderer: Renderer, tex: { prim: Texture; def: Texture; light: Texture; presence: Texture; atlas: Texture; atlas2?: Texture },
         originTileX: number, originTileY: number, debugTier = false, brute = false,
         dilateX = 2, win?: { cols: number; rows: number; level: number }): void {
    renderer.draw({
      program: this.prog, geometry: this.quad, target: this.a, blend: "none",
      textures: { uPrim: tex.prim, uDef: tex.def, uLight: tex.light, uPresence: tex.presence,
                  uPrev: this.b.textures[0], uSurfaceAtlas: tex.atlas, uSurfaceAtlas2: tex.atlas2 ?? tex.atlas },
      uniforms: (p) => {
        p.uVec2("uWindowOrigin", originTileX, originTileY);
        p.uInt("uUnitT", UNITS_PER_TILE >> (win?.level ?? 0));
        p.uInt("uCols", win?.cols ?? SLOTS_X); p.uInt("uRows", win?.rows ?? SLOTS_Y);
        p.uInt("uDebugTier", debugTier ? 1 : 0);
        p.uInt("uBrute", brute ? 1 : 0);
        p.uInt("uDilateX", dilateX);
      },
    });
    if (!debugTier && !brute) this.swap();
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

  destroy(): void { this.a.destroy(); this.b.destroy(); this.prog.destroy(); this.quad.destroy(); }
}
