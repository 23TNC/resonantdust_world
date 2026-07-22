//! shadowCaster (webgl) — lights + billboard shadows, cast as GPU-instanced projected geometry (the
//! `shadow-projection` stream). Aligns with the 5-triangle projected-silhouette model
//! (`docs/components/client/pixijs/design/shadows.md`, proven in `bin/shadow-projection-sandbox.html`) but
//! builds the fan **on the GPU, not the CPU**, reading all caster/light data from the `cold-data-textures`
//! GPU data textures via `texelFetch` (no per-frame instance attributes — the `caster-lut` C5 / webgl-engine
//! W7 payoff). One instanced draw per light (15 fan vertices × its `lut_count` casters); the vertex shader
//! fetches the light + LUT entry + prim def + placed instance by index, decodes the packed position, runs
//! `cornersWith`/`proj`, and emits the 15 fan vertices. `ColdShadowData` owns + builds the four textures.
//!
//! This slice: the **E/W regime** (all current cold things are single-facing → E/W), constant θ, silhouette
//! depth (`dA/dB` from `prim_definition_data`), alpha-masked. CPU keeps only the cheap on-change LUT build.
//!
//! GOTCHA: a backtick inside the GLSL closes the `/* glsl */` literal.

import { Renderer, Program, Geometry, Texture } from "../../gl";
import type { Camera } from "./Camera";
import type { Primitive } from "./SquareCache";
import type { TextureResolver } from "../../textures";
import { ColdShadowData } from "./coldShadowData";
import { SQUARE } from "./squareMath";

/** 6 lights this iteration. */
const MAX_LIGHTS = 6;
/** Default light ring around the seed tile, lit from above (world z). */
const LIGHT_Z = 480;
const LIGHT_RADIUS = 4 * SQUARE;
const RING_RADIUS = 2 * SQUARE;

interface Light {
  x: number;
  y: number;
  z: number;
  radius: number;
}

const CAST_VERT = /* glsl */ `#version 300 es
in float aVid;                 // 0..14 — which of the 5 triangles' 15 vertices
uniform highp usampler2D uLightData;  // light_data (128×33: row 0 record, rows 1–32 LUT)
uniform highp usampler2D uPrimDef;    // prim_definition_data (1/px)
uniform highp usampler2D uPrimData;   // prim_data (2/px)
uniform int  uLightIndex;      // which light (per-light draw) = the column
uniform int  uDefW;            // texture widths for index→texel
uniform int  uPrimW;
uniform mat3 uProjection;      // world px -> clip
out vec3 vColor;
out vec2 vUv;
flat out vec4 vFrame;

const int ROLE[15] = int[15](0,1,2, 0,3,2, 1,5,2, 0,4,2, 1,6,2);
const vec2 UV[7] = vec2[7](vec2(0.0,0.0), vec2(1.0,0.0), vec2(0.5,1.0), vec2(0.0,1.0), vec2(0.0,1.0), vec2(1.0,1.0), vec2(1.0,1.0));
const float UNIT = 4.0;        // SQUARE/16 (compile-time); world fields are in units
const uint  ZD = 16u, RD = 16u; // ZONE_DIM, REGION_DIM
const float ATLAS = 1024.0;    // atlas page size (F2: single page)

uvec4 fetchLin(highp usampler2D t, int i, int w) { return texelFetch(t, ivec2(i % w, i / w), 0); }
// position_anchor_reference (region|zone|tile|anchor) → world UNITS
vec2 decodePos(uint p) {
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

void main() {
  int role = ROLE[int(aVid + 0.5)];
  int k = uLightIndex;

  // Light record at column k, row 0.
  uvec4 Ld = texelFetch(uLightData, ivec2(k, 0), 0);
  vec3 L = vec3(decodePos(Ld.x), float((Ld.z >> 24) & 255u));         // pos (units) + z (units)
  vec3 col = vec3(float((Ld.y >> 24) & 255u), float((Ld.y >> 16) & 255u), float((Ld.y >> 8) & 255u)) / 255.0;

  // Caster j = gl_InstanceID → its u16 prim index from the LUT (rows 1–32, 8× u16/px, sentinel-0).
  int j = gl_InstanceID;
  uvec4 lrow = texelFetch(uLightData, ivec2(k, 1 + (j >> 3)), 0);
  uint packed = lrow[(j >> 1) & 3];
  int primIdx = int((j & 1) == 0 ? (packed >> 16) : (packed & 0xffffu));

  // Placed instance: position + def_index (rotation reserved for P5).
  uvec4 Pd = fetchLin(uPrimData, primIdx / 2, uPrimW);
  uint pos = (primIdx & 1) == 0 ? Pd.x : Pd.z;
  uint orient = (primIdx & 1) == 0 ? Pd.y : Pd.w;
  vec2 A = decodePos(pos);
  int defIdx = int((orient >> 6) & 0xffffu);

  // Generic def: geometry (units) + atlas frame (px) + base spread.
  uvec4 D = fetchLin(uPrimDef, defIdx, uDefW);
  float W = float((D.x >> 22) & 1023u), H = float((D.x >> 12) & 1023u);
  float fx = float((D.x >> 2) & 1023u), fw = float((D.y >> 22) & 1023u), fh = float((D.y >> 12) & 1023u), fy = float((D.y >> 2) & 1023u);
  float dA = float((D.z >> 14) & 255u), dB = float((D.z >> 6) & 255u);

  // E/W fan in UNITS (θ constant); ×UNIT to px only at the clip transform.
  float th = 65.0 * 3.14159265 / 180.0, ct = cos(th), st = sin(th);
  vec3 loc;
  if (role == 0)      loc = vec3(-W * 0.5, -0.5 * H * ct, H * st);
  else if (role == 1) loc = vec3( W * 0.5, -0.5 * H * ct, H * st);
  else if (role == 2) loc = vec3(0.0, 0.0, 0.0);
  else if (role == 3) loc = vec3(-W * 0.5, 0.0, 0.0);
  else if (role == 4) loc = vec3(-W * 0.5, -dA, 0.0);
  else if (role == 5) loc = vec3( W * 0.5, 0.0, 0.0);
  else                loc = vec3( W * 0.5, -dB, 0.0);

  vec2 g = projGround(vec3(A + loc.xy, loc.z), L);
  vec3 clip = uProjection * vec3(g * UNIT, 1.0);
  gl_Position = vec4(clip.xy, 0.0, 1.0);
  vColor = col;
  vUv = UV[role];
  vFrame = vec4(fx / ATLAS, fy / ATLAS, fw / ATLAS, fh / ATLAS);
}
`;
const CAST_FRAG = /* glsl */ `#version 300 es
precision highp float;
in vec3 vColor;
in vec2 vUv;
flat in vec4 vFrame;
uniform sampler2D uSurface;     // the casters' shared surface page (B = silhouette coverage)
out vec4 fragColor;
void main() {
  // Alpha mask: sample the sprite silhouette (surface.B) at this fragment's sprite uv, warped onto the
  // fan; discard outside the silhouette so the shadow reads as the caster's shape, not a solid polygon.
  if (vFrame.z > 0.0) {
    float cov = texture(uSurface, vFrame.xy + vUv * vFrame.zw).b;
    if (cov < 0.5) discard;
  }
  fragColor = vec4(vColor, 0.5);   // semi-transparent coloured shadow
}
`;

/** Six distinct per-light colours (the design's bit-decode palette, extended to 6). */
const LIGHT_COLORS: ReadonlyArray<[number, number, number]> = [
  [1.0, 0.25, 0.25], [0.25, 1.0, 0.3], [0.3, 0.55, 1.0], [1.0, 0.95, 0.25], [1.0, 0.35, 1.0], [0.3, 1.0, 1.0],
];

/**
 * Owns the light list + the instanced projected-fan shadow pass. The {@link Viewport} calls {@link tick}
 * each frame after the world display; casters come from the cold cache's standing prims.
 */
export class ShadowCaster {
  private readonly program: Program;
  private readonly geo: Geometry;
  private readonly lights: Light[] = [];
  /** 1×1 fallback bound to `uSurface` until a caster's surface page resolves (mask is a no-op then). */
  private readonly empty: Texture;
  private enabled = true;
  /** The GPU data textures (cold-data-textures) — def + prim + light + LUT, rebuilt only on change; the
   *  shader `texelFetch`es them (no instance attributes). */
  private readonly coldData: ColdShadowData;
  private lastCasterCount = -1;
  private coldDirty = true;
  /** Drops the resolver's onLoad subscription; a LOD landing re-dirties the cold build so newly-resolved
   *  sprites get their defs (else the first pre-texture build sticks with 0 casters). */
  private resolverUnsub: (() => void) | null = null;

  constructor(private readonly renderer: Renderer) {
    const gl = renderer.gl;
    this.program = new Program(gl, CAST_VERT, CAST_FRAG, "viewport-shadow-cast");
    this.empty = new Texture(gl, { width: 1, height: 1, data: new Uint8Array([0, 0, 0, 0]) });
    this.coldData = new ColdShadowData(renderer);
    (globalThis as unknown as { __cold: unknown }).__cold = this.coldData; // DEBUG (cold-data-textures P1)
    // aVid drives count=15 (the fan); instances = a light's lut_count (set per draw). All caster/light data
    // is read from the cold data textures via texelFetch — no instance attributes.
    const vid = new Float32Array([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]);
    this.geo = new Geometry(gl, this.program, { aVid: { data: vid, size: 1 } }, undefined, 0);
    this.seed(100, 50);
  }

  /** Seed 6 lights in a ring around a tile (world px), matching the shadows-stream default (100, 50). */
  seed(tileX: number, tileY: number): void {
    const cx = (tileX + 0.5) * SQUARE;
    const cy = (tileY + 0.5) * SQUARE;
    this.lights.length = 0;
    for (let k = 0; k < MAX_LIGHTS; k++) {
      const a = (k / MAX_LIGHTS) * Math.PI * 2;
      this.lights.push({ x: cx + Math.cos(a) * RING_RADIUS, y: cy + Math.sin(a) * RING_RADIUS, z: LIGHT_Z, radius: LIGHT_RADIUS });
    }
    this.enabled = true;
    this.coldDirty = true; // lights changed → rebuild the cold light/LUT textures next tick
  }

  get on(): boolean {
    return this.enabled;
  }
  toggle(): boolean {
    this.enabled = !this.enabled;
    return this.enabled;
  }

  /** Draw the shadows over the world display. `standing` = the cold cache's standing prims (things);
   *  `resolver` resolves each caster's sprite (for the def/depth/mask). The fan is built entirely on the GPU
   *  from the four cold data textures — no per-frame instance attributes. */
  tick(camera: Camera, standing: Primitive[], resolver: TextureResolver | null): void {
    if (!this.enabled || this.lights.length === 0) return;
    const nLights = Math.min(this.lights.length, MAX_LIGHTS);

    // A LOD landing re-dirties the build (so sprites resolving after the first build get their defs).
    if (!this.resolverUnsub && resolver) this.resolverUnsub = resolver.onLoad(() => { this.coldDirty = true; });

    // Build the cold data textures only on change (caster set / lights / a resolved sprite) — NOT per frame
    // (cold data is static). Populates def + prim + light + LUT.
    if (this.coldDirty || standing.length !== this.lastCasterCount) {
      const coldLights = this.lights.slice(0, nLights).map((L, k) => ({
        x: L.x, y: L.y, z: L.z, radius: L.radius, color: LIGHT_COLORS[k], intensity: 1, castShadows: true,
      }));
      this.coldData.buildLights(coldLights, standing, resolver);
      this.lastCasterCount = standing.length;
      this.coldDirty = false;
    }
    if (this.coldData.lights === 0) return;

    const w = camera.width, h = camera.height, z = camera.zoom, ax = camera.anchorX, ay = camera.anchorY;
    // world px → clip (pan + zoom + screen→clip; screen y-down → clip y-up), column-major mat3.
    const proj = new Float32Array([(2 * z) / w, 0, 0, 0, -(2 * z) / h, 0, (-ax * 2 * z) / w, (ay * 2 * z) / h, 1]);
    const [, defW, primW] = this.coldData.widths;
    const surface = this.coldData.surfacePage ?? this.empty;
    // One instanced draw per light: 15 fan vertices × its caster count, the vertex shader texelFetch-ing the
    // light record + LUT run (in light_data rows) + prim + def by index. Association is inherent (per-column).
    for (let k = 0; k < this.coldData.lights; k++) {
      const count = this.coldData.casterCount(k);
      if (count === 0) continue; // non-casting light / no in-range casters
      this.geo.instanceCount = count;
      this.renderer.draw({
        program: this.program,
        geometry: this.geo,
        blend: "normal",
        textures: {
          uLightData: this.coldData.lightTexture,
          uPrimDef: this.coldData.definitionTexture,
          uPrimData: this.coldData.primTexture,
          uSurface: surface,
        },
        uniforms: (p) => {
          p.uMat3("uProjection", proj);
          p.uInt("uLightIndex", k);
          p.uInt("uDefW", defW);
          p.uInt("uPrimW", primW);
        },
      });
    }
  }

  destroy(): void {
    this.resolverUnsub?.();
    this.geo.destroy();
    this.program.destroy();
    this.empty.destroy();
    this.coldData.destroy();
  }
}
