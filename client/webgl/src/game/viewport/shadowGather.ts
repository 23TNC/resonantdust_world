//! shadowGather (webgl) — the per-light **shadow bitfield** via a fragment **gather**
//! (`2026-07-21-shadow-bitfield`). Each cold light is one bit in a world-space toroidal `RGBA32UI`
//! `shadow-cold` buffer; a fragment computes its texel's full 128-bit mask by looping the reaching
//! lights, testing point-in-projected-silhouette against each light's casters, and OR-ing bits in a
//! register — one write, no ping-pong (F1). Data comes from `ColdShadowData` (`light_data` / `prim_data`
//! / `prim_definition_data`) via `texelFetch`. Layouts authoritative in `docs/VARIABLES.md`.
//!
//! Build order (deviation D-1): this slice is **P4-core** — full recompute each frame, **all** lights
//! (no presence cull yet), the exact fan region (roles 0–6 → 5 triangles) as the per-fragment predicate
//! (no silhouette mask yet). Presence cull (P2), dirty-tile gating (P3) and the silhouette mask layer on
//! next as invisible optimisations. The projected-region maths is a direct port of `shadowCaster.ts`.
//!
//! GOTCHA: a backtick inside the GLSL closes the `/* glsl */` literal.

import { Renderer, Program, Geometry, RenderTarget, Texture } from "../../gl";
import type { Camera } from "./Camera";
import type { Primitive } from "./SquareCache";
import type { TextureResolver } from "../../textures";
import { ColdShadowData, MAX_CASTERS } from "./coldShadowData";
import { SQUARE } from "./squareMath";

/** 6 lights this iteration (ring around the seed tile) — same seed as the retired fan. */
const MAX_LIGHTS = 6;
const LIGHT_Z = 480;
const LIGHT_RADIUS = 4 * SQUARE;
const RING_RADIUS = 2 * SQUARE;
/** shadow-cold resolution: `SHADOW_SLOT` texels per world tile (a tile is `SQUARE` world px). 16 = 1
 *  texel/world-px. Sized `cols·SHADOW_SLOT × rows·SHADOW_SLOT`, toroidal like the cold cache window. */
const SHADOW_SLOT = 16;
/** GLSL literals for the world constants (a tile is `SQUARE` world px; `1 unit = SQUARE/16`). */
const SQF = SQUARE.toFixed(1);
const UNITF = (SQUARE / 16).toFixed(4);

interface Light {
  x: number;
  y: number;
  z: number;
  radius: number;
}

/** The cold cache's toroidal tile window (from `SquareCache.window`) — shadow-cold aligns to it. */
export interface TileWindow {
  winCol: number;
  winRow: number;
  cols: number;
  rows: number;
}

/** Shared GLSL: packed-position decode + ground projection + the fan region predicate (roles → tris). */
const GATHER_COMMON = /* glsl */ `
const float UNIT = ${UNITF};      // SQUARE/16 (compile-time; px per unit)
const float SQ = ${SQF};          // SQUARE world px per tile
const uint  ZD = 16u, RD = 16u;  // ZONE_DIM, REGION_DIM
uvec4 fetchLin(highp usampler2D t, int i, int w) { return texelFetch(t, ivec2(i % w, i / w), 0); }
vec2 decodePos(uint p) {          // position_anchor_reference → world UNITS
  uint region = (p >> 24) & 255u, zone = (p >> 16) & 255u, tile = (p >> 8) & 255u, anchor = p & 255u;
  uint wtx = (((region >> 4u) * RD + (zone >> 4u)) * ZD + (tile >> 4u));
  uint wty = (((region & 15u) * RD + (zone & 15u)) * ZD + (tile & 15u));
  return vec2(float(wtx * 16u + (anchor >> 4u)), float(wty * 16u + (anchor & 15u)));
}
vec2 projGround(vec3 p, vec3 L) {
  if (p.z <= 0.0) return p.xy;
  float t = min(L.z / max(L.z - p.z, 1.0), 8.0);
  return L.xy + t * (p.xy - L.xy);
}
float cross2(vec2 a, vec2 b) { return a.x * b.y - a.y * b.x; }
bool inTri(vec2 p, vec2 a, vec2 b, vec2 c) {
  float d1 = cross2(b - a, p - a), d2 = cross2(c - b, p - b), d3 = cross2(a - c, p - c);
  bool neg = (d1 < 0.0) || (d2 < 0.0) || (d3 < 0.0);
  bool pos = (d1 > 0.0) || (d2 > 0.0) || (d3 > 0.0);
  return !(neg && pos);
}
// Is world-UNIT point P inside caster (anchor A, geometry W/H/dA/dB) projected from light L? The exact fan
// region: 7 projected points (roles 0–6) → union of the 5 fan triangles.
bool inShadow(vec2 P, vec2 A, vec3 L, float W, float H, float dA, float dB) {
  float th = 65.0 * 3.14159265 / 180.0, ct = cos(th), st = sin(th);
  vec3 loc[7];
  loc[0] = vec3(-W * 0.5, -0.5 * H * ct, H * st);
  loc[1] = vec3( W * 0.5, -0.5 * H * ct, H * st);
  loc[2] = vec3(0.0, 0.0, 0.0);
  loc[3] = vec3(-W * 0.5, 0.0, 0.0);
  loc[4] = vec3(-W * 0.5, -dA, 0.0);
  loc[5] = vec3( W * 0.5, 0.0, 0.0);
  loc[6] = vec3( W * 0.5, -dB, 0.0);
  vec2 g[7];
  for (int i = 0; i < 7; i++) g[i] = projGround(vec3(A + loc[i].xy, loc[i].z), L);
  return inTri(P, g[0], g[1], g[2]) || inTri(P, g[0], g[3], g[2]) || inTri(P, g[1], g[5], g[2])
      || inTri(P, g[0], g[4], g[2]) || inTri(P, g[1], g[6], g[2]);
}
`;

const FULLSCREEN_VERT = /* glsl */ `#version 300 es
in vec2 aPos;                    // NDC -1..1 (2 tris)
void main() { gl_Position = vec4(aPos, 0.0, 1.0); }
`;

const GATHER_FRAG = /* glsl */ `#version 300 es
precision highp float;
precision highp int;
uniform highp usampler2D uLightData;  // light_data (128×33)
uniform highp usampler2D uPrimDef;    // prim_definition_data
uniform highp usampler2D uPrimData;   // prim_data (2/px)
uniform int uNLights, uDefW, uPrimW;
uniform int uCols, uRows, uWinCol, uWinRow, uSlot;   // shadow-cold toroidal window
out uvec4 fragColor;
${GATHER_COMMON}
int pmod(int a, int m) { return ((a % m) + m) % m; }
void main() {
  ivec2 fc = ivec2(gl_FragCoord.xy);
  int sx = fc.x / uSlot, sy = fc.y / uSlot;                 // toroidal slot
  int wc = uWinCol + pmod(sx - pmod(uWinCol, uCols), uCols); // → world tile
  int wr = uWinRow + pmod(sy - pmod(uWinRow, uRows), uRows);
  float lx = (float(fc.x) - float(sx * uSlot)) / float(uSlot); // 0..1 within the tile
  float ly = (float(fc.y) - float(sy * uSlot)) / float(uSlot);
  vec2 P = vec2((float(wc) + lx) * SQ, (float(wr) + ly) * SQ) / UNIT; // world UNITS

  uint b0 = 0u, b1 = 0u, b2 = 0u, b3 = 0u;
  for (int k = 0; k < uNLights; k++) {
    uvec4 Ld = texelFetch(uLightData, ivec2(k, 0), 0);
    if ((Ld.z & 1u) == 0u) continue;                        // cast_shadows
    vec3 L = vec3(decodePos(Ld.x), float((Ld.z >> 24) & 255u));
    for (int c = 0; c < ${MAX_CASTERS}; c++) {
      uvec4 lrow = texelFetch(uLightData, ivec2(k, 1 + (c >> 3)), 0);
      uint packed = lrow[(c >> 1) & 3];
      uint primIdx = (c & 1) == 0 ? (packed >> 16) : (packed & 0xffffu);
      if (primIdx == 0u) break;                             // sentinel → end of run
      uvec4 Pd = fetchLin(uPrimData, int(primIdx) / 2, uPrimW);
      uint pos = (primIdx & 1u) == 0u ? Pd.x : Pd.z;
      uint orient = (primIdx & 1u) == 0u ? Pd.y : Pd.w;
      vec2 A = decodePos(pos);
      int defIdx = int((orient >> 6) & 0xffffu);
      uvec4 D = fetchLin(uPrimDef, defIdx, uDefW);
      float W = float((D.x >> 22) & 1023u), H = float((D.x >> 12) & 1023u);
      float dA = float((D.z >> 14) & 255u), dB = float((D.z >> 6) & 255u);
      if (inShadow(P, A, L, W, H, dA, dB)) {
        uint mask = 1u << uint(k & 31);
        int ch = k >> 5;
        if (ch == 0) b0 |= mask; else if (ch == 1) b1 |= mask; else if (ch == 2) b2 |= mask; else b3 |= mask;
        break;                                              // this light shadows P → next light
      }
    }
  }
  fragColor = uvec4(b0, b1, b2, b3);
}
`;

/** Overlay: sample shadow-cold at the fragment's world position, decode set bits → per-light colours. */
const OVERLAY_VERT = /* glsl */ `#version 300 es
in vec2 aWorld;                  // world px (window rect)
uniform mat3 uProjection;
out vec2 vWorld;
void main() {
  vWorld = aWorld;
  vec3 p = uProjection * vec3(aWorld, 1.0);
  gl_Position = vec4(p.xy, 0.0, 1.0);
}
`;
const OVERLAY_FRAG = /* glsl */ `#version 300 es
precision highp float;
precision highp int;
in vec2 vWorld;
uniform highp usampler2D uShadow;   // shadow-cold (RGBA32UI)
uniform int uCols, uRows, uWinCol, uWinRow, uSlot;
out vec4 fragColor;
int pmod(int a, int m) { return ((a % m) + m) % m; }
// Distinct-ish colour per bit index (hue wheel); overlap sums.
vec3 hueColour(int k) {
  float h = fract(float(k) * 0.61803398875);   // golden-ratio hue spread
  vec3 c = clamp(abs(fract(h + vec3(0.0, 0.6666, 0.3333)) * 6.0 - 3.0) - 1.0, 0.0, 1.0);
  return c;
}
void main() {
  int tx = int(floor(vWorld.x / ${SQF})), ty = int(floor(vWorld.y / ${SQF}));
  if (tx < uWinCol || tx >= uWinCol + uCols || ty < uWinRow || ty >= uWinRow + uRows) { fragColor = vec4(0.0); return; }
  int sx = pmod(tx, uCols), sy = pmod(ty, uRows);
  float lx = fract(vWorld.x / ${SQF}), ly = fract(vWorld.y / ${SQF});
  ivec2 texel = ivec2(sx * uSlot + int(lx * float(uSlot)), sy * uSlot + int(ly * float(uSlot)));
  uvec4 bits = texelFetch(uShadow, texel, 0);
  vec3 acc = vec3(0.0);
  int set = 0;
  for (int k = 0; k < 128; k++) {
    uint word = k < 32 ? bits.x : (k < 64 ? bits.y : (k < 96 ? bits.z : bits.w));
    if ((word & (1u << uint(k & 31))) != 0u) { acc += hueColour(k); set++; }
  }
  if (set == 0) { fragColor = vec4(0.0); return; }
  fragColor = vec4(clamp(acc, 0.0, 1.0), 1.0);
}
`;

const LIGHT_COLORS: ReadonlyArray<[number, number, number]> = [
  [1.0, 0.25, 0.25], [0.25, 1.0, 0.3], [0.3, 0.55, 1.0], [1.0, 0.95, 0.25], [1.0, 0.35, 1.0], [0.3, 1.0, 1.0],
];

/**
 * Owns the light list, the cold data textures, the world-space toroidal `shadow-cold` bitfield, and the
 * gather pass that writes it. {@link tick} recomputes the bitfield each frame (P4-core); {@link drawOverlay}
 * decodes it over the world for `/overlayRT shadow-cold`.
 */
export class ShadowGather {
  private readonly gather: Program;
  private readonly overlay: Program;
  private readonly fsQuad: Geometry;
  private overlayGeo: Geometry | null = null;
  private readonly overlayPos = new Float32Array(8);
  private readonly lights: Light[] = [];
  private readonly empty: Texture;
  private enabled = true;
  private readonly coldData: ColdShadowData;
  private lastCasterCount = -1;
  private coldDirty = true;
  private resolverUnsub: (() => void) | null = null;

  /** shadow-cold RT (world-space toroidal RGBA32UI), resized when the tile window changes. */
  private shadowRT: RenderTarget | null = null;
  private rtCols = 0;
  private rtRows = 0;

  constructor(private readonly renderer: Renderer) {
    const gl = renderer.gl;
    this.gather = new Program(gl, FULLSCREEN_VERT, GATHER_FRAG, "shadow-gather");
    this.overlay = new Program(gl, OVERLAY_VERT, OVERLAY_FRAG, "shadow-overlay");
    this.empty = new Texture(gl, { width: 1, height: 1, data: new Uint8Array([0, 0, 0, 0]) });
    this.coldData = new ColdShadowData(renderer);
    (globalThis as unknown as { __cold: unknown }).__cold = this.coldData; // DEBUG
    this.fsQuad = new Geometry(gl, this.gather, {
      aPos: { data: new Float32Array([-1, -1, 1, -1, 1, 1, -1, 1]), size: 2 },
    }, new Uint32Array([0, 1, 2, 0, 2, 3]));
    this.overlayGeo = new Geometry(gl, this.overlay, {
      aWorld: { data: this.overlayPos, size: 2 },
    }, new Uint32Array([0, 1, 2, 0, 2, 3]));
    this.seed(100, 50);
  }

  /** Seed 6 lights in a ring around a tile (world px). */
  seed(tileX: number, tileY: number): void {
    const cx = (tileX + 0.5) * SQUARE, cy = (tileY + 0.5) * SQUARE;
    this.lights.length = 0;
    for (let k = 0; k < MAX_LIGHTS; k++) {
      const a = (k / MAX_LIGHTS) * Math.PI * 2;
      this.lights.push({ x: cx + Math.cos(a) * RING_RADIUS, y: cy + Math.sin(a) * RING_RADIUS, z: LIGHT_Z, radius: LIGHT_RADIUS });
    }
    this.enabled = true;
    this.coldDirty = true;
  }

  get on(): boolean {
    return this.enabled;
  }
  toggle(): boolean {
    this.enabled = !this.enabled;
    return this.enabled;
  }

  private ensureRT(cols: number, rows: number): void {
    if (this.shadowRT && cols === this.rtCols && rows === this.rtRows) return;
    this.shadowRT?.destroy();
    this.shadowRT = new RenderTarget(this.renderer.gl, {
      width: Math.max(1, cols * SHADOW_SLOT), height: Math.max(1, rows * SHADOW_SLOT), formats: ["rgba32uint"],
    });
    this.rtCols = cols;
    this.rtRows = rows;
  }

  /** Rebuild the cold data (on change) + recompute the shadow-cold bitfield for the window. */
  tick(standing: Primitive[], resolver: TextureResolver | null, win: TileWindow): void {
    if (!this.enabled || this.lights.length === 0) return;
    if (!this.resolverUnsub && resolver) this.resolverUnsub = resolver.onLoad(() => { this.coldDirty = true; });

    if (this.coldDirty || standing.length !== this.lastCasterCount) {
      const coldLights = this.lights.slice(0, MAX_LIGHTS).map((L, k) => ({
        x: L.x, y: L.y, z: L.z, radius: L.radius, color: LIGHT_COLORS[k], intensity: 1, castShadows: true,
      }));
      this.coldData.buildLights(coldLights, standing, resolver);
      this.lastCasterCount = standing.length;
      this.coldDirty = false;
    }
    if (this.coldData.lights === 0 || win.cols === 0) return;

    this.ensureRT(win.cols, win.rows);
    const [, defW, primW] = this.coldData.widths;
    // Full recompute (P4-core): one fullscreen pass writes every texel's 128-bit mask.
    this.renderer.draw({
      program: this.gather,
      geometry: this.fsQuad,
      target: this.shadowRT!,
      blend: "none",
      clearInt: [0, 0, 0, 0],
      textures: {
        uLightData: this.coldData.lightTexture,
        uPrimDef: this.coldData.definitionTexture,
        uPrimData: this.coldData.primTexture,
      },
      uniforms: (p) => {
        p.uInt("uNLights", this.coldData.lights);
        p.uInt("uDefW", defW);
        p.uInt("uPrimW", primW);
        p.uInt("uCols", win.cols);
        p.uInt("uRows", win.rows);
        p.uInt("uWinCol", win.winCol);
        p.uInt("uWinRow", win.winRow);
        p.uInt("uSlot", SHADOW_SLOT);
      },
    });
  }

  /** Decode shadow-cold over the world (debug `/overlayRT shadow-cold`). */
  drawOverlay(camera: Camera, win: TileWindow): void {
    if (!this.shadowRT || win.cols === 0) return;
    const w = camera.width, h = camera.height, z = camera.zoom, ax = camera.anchorX, ay = camera.anchorY;
    // Cover the window's world rect.
    const x0 = win.winCol * SQUARE, y0 = win.winRow * SQUARE;
    const x1 = (win.winCol + win.cols) * SQUARE, y1 = (win.winRow + win.rows) * SQUARE;
    this.overlayPos.set([x0, y0, x1, y0, x1, y1, x0, y1]);
    this.overlayGeo!.update("aWorld", this.overlayPos);
    const proj = new Float32Array([(2 * z) / w, 0, 0, 0, -(2 * z) / h, 0, (-ax * 2 * z) / w, (ay * 2 * z) / h, 1]);
    this.renderer.draw({
      program: this.overlay,
      geometry: this.overlayGeo!,
      blend: "normal",
      textures: { uShadow: this.shadowRT.textures[0] },
      uniforms: (p) => {
        p.uMat3("uProjection", proj);
        p.uInt("uCols", win.cols);
        p.uInt("uRows", win.rows);
        p.uInt("uWinCol", win.winCol);
        p.uInt("uWinRow", win.winRow);
        p.uInt("uSlot", SHADOW_SLOT);
      },
    });
  }

  destroy(): void {
    this.resolverUnsub?.();
    this.fsQuad.destroy();
    this.overlayGeo?.destroy();
    this.gather.destroy();
    this.overlay.destroy();
    this.empty.destroy();
    this.shadowRT?.destroy();
    this.coldData.destroy();
  }
}
