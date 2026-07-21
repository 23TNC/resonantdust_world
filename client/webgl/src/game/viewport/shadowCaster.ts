//! shadowCaster (webgl) — lights + billboard shadows, cast as GPU-instanced projected geometry (the
//! `shadow-projection` stream). Aligns with the 5-triangle projected-silhouette model
//! (`docs/components/client/pixijs/design/shadows.md`, proven in `bin/shadow-projection-sandbox.html`) but
//! builds the fan **on the GPU, not the CPU**: one instance per (light, caster) pair, and the vertex shader
//! reads the instance's caster (`Ax,Ay,W,H,θ,dA,dB`) + light (`Lx,Ly,Lz`), runs `cornersWith` + `proj`
//! per-corner, and emits the 15 fan vertices. No per-frame CPU geometry rebuild — this is the `caster-lut`
//! C5 / webgl-engine W7 payoff on the owned engine.
//!
//! This slice: the **E/W regime** (all current cold things are single-facing → E/W), a constant ground
//! angle θ, and a rough depth (P3 replaces it with the sprite-silhouette presence bake); solid coloured
//! triangles (P4 adds the alpha-mask fragment). CPU keeps only the cheap in-range (light, caster) pairing.
//!
//! GOTCHA: a backtick inside the GLSL closes the `/* glsl */` literal.

import { Renderer, Program, Geometry, Texture } from "../../gl";
import type { Camera } from "./Camera";
import type { Primitive } from "./SquareCache";
import type { TextureResolver } from "../../textures";
import { SQUARE } from "./squareMath";

/** 6 lights this iteration; casters capped so a dense zone can't blow the instance buffer. */
const MAX_LIGHTS = 6;
const MAX_PAIRS = 4096;
/** Default light ring around the seed tile, lit from above (world z). */
const LIGHT_Z = 480;
const LIGHT_RADIUS = 4 * SQUARE;
const RING_RADIUS = 2 * SQUARE;
/** Ground angle θ (E/W side-on) — the art-style oblique tilt (sandbox default 65°). Constant for now. */
const THETA = (65 * Math.PI) / 180;
/** Fallback base-spread depth as a fraction of the billboard width, until the presence bake resolves. */
const DEPTH_FRAC = 0.22;

interface Light {
  x: number;
  y: number;
  z: number;
  radius: number;
}

const CAST_VERT = /* glsl */ `#version 300 es
in float aVid;                 // 0..14 — which of the 5 triangles' 15 vertices
in vec4 aCaster;               // Ax, Ay (ground anchor world px), W, H (billboard px)
in vec4 aParam;                // theta, dA, dB, (unused)
in vec3 aLight;                // Lx, Ly, Lz (world px; z = height)
in vec3 aColor;                // this light's colour
in vec4 aFrame;                // surface atlas sub-frame uv rect [u0,v0,uw,vh] (zw=0 → no mask, solid)
uniform mat3 uProjection;      // world px -> clip
out vec3 vColor;
out vec2 vUv;                  // sprite uv for the alpha mask
flat out vec4 vFrame;

// gl_VertexID role table. Roles: 0=TL 1=TR 2=BC 3=BL+ 4=BL- 5=BR+ 6=BR-.
// T1(TL,TR,BC) T2(TL,BL+,BC) T3(TR,BR+,BC) T4(TL,BL-,BC) T5(TR,BR-,BC).
const int ROLE[15] = int[15](0,1,2, 0,3,2, 1,5,2, 0,4,2, 1,6,2);
// Sprite uv per role (E/W bottom-edge sampling): the shadow reads the whole sprite silhouette warped
// onto the fan — top (v=0) → the projected top corners, base (v=1) → the footprint corners.
const vec2 UV[7] = vec2[7](vec2(0.0,0.0), vec2(1.0,0.0), vec2(0.5,1.0), vec2(0.0,1.0), vec2(0.0,1.0), vec2(1.0,1.0), vec2(1.0,1.0));

// Per-corner radial projection to the ground (z=0); sub-ground drops straight down (footprint).
vec2 projGround(vec3 p, vec3 L) {
  if (p.z <= 0.0) return p.xy;
  float t = min(L.z / max(L.z - p.z, 1.0), 8.0);   // TMAX = 8 (clamp grazing runaways)
  return L.xy + t * (p.xy - L.xy);
}

void main() {
  int role = ROLE[int(aVid + 0.5)];
  vec2 A = aCaster.xy; float W = aCaster.z, H = aCaster.w;
  float th = aParam.x, dA = aParam.y, dB = aParam.z;
  float ct = cos(th), st = sin(th);

  // E/W billboard, tilted by θ (leans north = -y as it rises in z); base rooted to the footprint.
  // TL/TR from cornersWith (centre-frame y); base corners overridden to the ground edge; ± depth spread
  // (E/W offsets on y, + variant = +0 seats on the foot, - variant spreads north).
  vec3 loc;
  if (role == 0)      loc = vec3(-W * 0.5, -0.5 * H * ct, H * st);   // TL (projected)
  else if (role == 1) loc = vec3( W * 0.5, -0.5 * H * ct, H * st);   // TR (projected)
  else if (role == 2) loc = vec3(0.0, 0.0, 0.0);                     // BC
  else if (role == 3) loc = vec3(-W * 0.5, 0.0, 0.0);                // BL+
  else if (role == 4) loc = vec3(-W * 0.5, -dA, 0.0);               // BL-
  else if (role == 5) loc = vec3( W * 0.5, 0.0, 0.0);                // BR+
  else                loc = vec3( W * 0.5, -dB, 0.0);               // BR-

  vec3 world = vec3(A + loc.xy, loc.z);
  vec2 g = projGround(world, aLight);
  vec3 clip = uProjection * vec3(g, 1.0);
  gl_Position = vec4(clip.xy, 0.0, 1.0);
  vColor = aColor;
  vUv = UV[role];
  vFrame = aFrame;
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
  // Per-instance buffers (one (light, caster) pair each), grown as needed.
  private aCaster = new Float32Array(0);
  private aParam = new Float32Array(0);
  private aLight = new Float32Array(0);
  private aColor = new Float32Array(0);
  private aFrame = new Float32Array(0);
  /** 1×1 fallback bound to `uSurface` when no caster's surface page is available (all instances solid). */
  private readonly empty: Texture;
  private enabled = true;
  /** Silhouette-derived base depth `[dA, dB]` per sprite stem (the sandbox's auto rule), baked once from
   *  the surface coverage on first sight + cached; stems still resolving fall back to {@link DEPTH_FRAC}. */
  private readonly depthCache = new Map<string, [number, number]>();

  constructor(private readonly renderer: Renderer) {
    const gl = renderer.gl;
    this.program = new Program(gl, CAST_VERT, CAST_FRAG, "viewport-shadow-cast");
    this.empty = new Texture(gl, { width: 1, height: 1, data: new Uint8Array([0, 0, 0, 0]) });
    // aVid drives count=15 (non-instanced); the instance attrs advance once per pair.
    const vid = new Float32Array([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]);
    this.geo = new Geometry(
      gl,
      this.program,
      {
        aVid: { data: vid, size: 1 },
        aCaster: { data: new Float32Array(4), size: 4, instanced: true },
        aParam: { data: new Float32Array(4), size: 4, instanced: true },
        aLight: { data: new Float32Array(3), size: 3, instanced: true },
        aColor: { data: new Float32Array(3), size: 3, instanced: true },
        aFrame: { data: new Float32Array(4), size: 4, instanced: true },
      },
      undefined,
      0,
    );
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
  }

  get on(): boolean {
    return this.enabled;
  }
  toggle(): boolean {
    this.enabled = !this.enabled;
    return this.enabled;
  }

  /** Draw the shadows over the world display. `standing` = the cold cache's standing prims (things);
   *  `resolver` supplies each caster's surface silhouette for the depth bake (P3). */
  tick(camera: Camera, standing: Primitive[], resolver: TextureResolver | null): void {
    if (!this.enabled || this.lights.length === 0) return;
    const nLights = Math.min(this.lights.length, MAX_LIGHTS);

    // CPU keeps only the cheap part: the in-range (light, caster) pair list. Everything geometric is GPU.
    let n = 0;
    const caster = this.grow("aCaster", standing.length * nLights * 4, 4);
    const param = this.grow("aParam", standing.length * nLights * 4, 4);
    const light = this.grow("aLight", standing.length * nLights * 3, 3);
    const color = this.grow("aColor", standing.length * nLights * 3, 3);
    const frame = this.grow("aFrame", standing.length * nLights * 4, 4);
    let surfacePage: Texture | null = null;
    for (const p of standing) {
      const Ax = p.x + p.width * 0.5; // ground anchor (bottom-centre of the sprite box)
      const Ay = p.y + p.height;
      // Base spread from the sprite silhouette (auto rule), or the rough fallback until it's baked.
      const d = this.depthFor(p, resolver);
      const dA = d ? d[0] : p.width * DEPTH_FRAC;
      const dB = d ? d[1] : p.width * DEPTH_FRAC;
      // The surface silhouette sub-frame for the alpha mask (all casters' surfaces share one atlas page at
      // a LOD; capture it to bind). zw=0 → no mask (solid) until the surface LOD resolves.
      const surf = p.textureName && resolver ? resolver.resolve(p.textureName, "surface", p.cell) : null;
      const uv = surf && !surf.geo && surf.frame ? surf.frame.uvRect() : null;
      if (surf?.frame) surfacePage = surf.frame.source;
      for (let k = 0; k < nLights; k++) {
        if (n >= MAX_PAIRS) break;
        const L = this.lights[k];
        if (Math.hypot(Ax - L.x, Ay - L.y) > L.radius) continue; // out of this light's reach
        caster[n * 4] = Ax; caster[n * 4 + 1] = Ay; caster[n * 4 + 2] = p.width; caster[n * 4 + 3] = p.height;
        param[n * 4] = THETA; param[n * 4 + 1] = dA; param[n * 4 + 2] = dB; param[n * 4 + 3] = 0;
        light[n * 3] = L.x; light[n * 3 + 1] = L.y; light[n * 3 + 2] = L.z;
        const c = LIGHT_COLORS[k];
        color[n * 3] = c[0]; color[n * 3 + 1] = c[1]; color[n * 3 + 2] = c[2];
        if (uv) { frame[n * 4] = uv[0]; frame[n * 4 + 1] = uv[1]; frame[n * 4 + 2] = uv[2]; frame[n * 4 + 3] = uv[3]; }
        else { frame[n * 4] = 0; frame[n * 4 + 1] = 0; frame[n * 4 + 2] = 0; frame[n * 4 + 3] = 0; }
        n++;
      }
    }
    if (n === 0) return;

    // Upload the N pairs + draw N instances × 15 vertices (5 triangles each).
    this.geo.update("aCaster", caster.subarray(0, n * 4));
    this.geo.update("aParam", param.subarray(0, n * 4));
    this.geo.update("aLight", light.subarray(0, n * 3));
    this.geo.update("aColor", color.subarray(0, n * 3));
    this.geo.update("aFrame", frame.subarray(0, n * 4));
    this.geo.instanceCount = n;

    const w = camera.width, h = camera.height, z = camera.zoom, ax = camera.anchorX, ay = camera.anchorY;
    // world px → clip (pan + zoom + screen→clip; screen y-down → clip y-up), column-major mat3.
    const proj = new Float32Array([(2 * z) / w, 0, 0, 0, -(2 * z) / h, 0, (-ax * 2 * z) / w, (ay * 2 * z) / h, 1]);
    this.renderer.draw({
      program: this.program,
      geometry: this.geo,
      blend: "normal",
      textures: { uSurface: surfacePage ?? this.empty },
      uniforms: (prog) => prog.uMat3("uProjection", proj),
    });
  }

  /** Silhouette base depth `[dA, dB]` for a caster, baked ONCE from its surface coverage + cached. Returns
   *  null until the sprite's `surface` LOD has resolved (the caller falls back to the rough default). The
   *  rule (E/W, `design/shadows.md` §Depth-from-presence): `depth = ½·(avg opaque HEIGHT of the half's
   *  columns / TS)·H`, left half → `dA`, right half → `dB`. */
  private depthFor(prim: Primitive, resolver: TextureResolver | null): [number, number] | null {
    if (!prim.textureName || !resolver) return null;
    const cached = this.depthCache.get(prim.textureName);
    if (cached) return cached;
    const surf = resolver.resolve(prim.textureName, "surface", prim.cell);
    if (surf.geo || !surf.frame) return null; // not loaded yet — rough fallback this frame

    // Read back the surface frame's B channel (coverage) into a presence bitmap.
    const gl = this.renderer.gl;
    const f = surf.frame;
    const w = f.w, h = f.h;
    const fb = gl.createFramebuffer();
    gl.bindFramebuffer(gl.FRAMEBUFFER, fb);
    gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, f.source.handle, 0);
    const px = new Uint8Array(w * h * 4);
    gl.readPixels(f.x, f.y, w, h, gl.RGBA, gl.UNSIGNED_BYTE, px);
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    gl.deleteFramebuffer(fb);
    const present = (x: number, y: number): boolean => px[(y * w + x) * 4 + 2] > 127; // B = coverage

    // Avg opaque HEIGHT (top→bottom span) of columns [x0, x1).
    const avgHeight = (x0: number, x1: number): number => {
      let sum = 0, cols = 0;
      for (let x = x0; x < x1; x++) {
        let top = -1, bot = -1;
        for (let y = 0; y < h; y++) if (present(x, y)) { top = y; break; }
        for (let y = h - 1; y >= 0; y--) if (present(x, y)) { bot = y; break; }
        if (top >= 0) { sum += bot - top + 1; cols++; }
      }
      return cols ? sum / cols : 0;
    };
    const mid = w >> 1;
    const H = prim.height;
    const dA = 0.5 * (avgHeight(0, mid) / h) * H;
    const dB = 0.5 * (avgHeight(mid, w) / h) * H;
    const out: [number, number] = [dA, dB];
    this.depthCache.set(prim.textureName, out);
    return out;
  }

  /** Grow the named instance buffer to at least `len` floats, returning a working array. */
  private grow(name: "aCaster" | "aParam" | "aLight" | "aColor" | "aFrame", len: number, _size: number): Float32Array {
    const cur = this[name];
    if (cur.length >= len) return cur;
    const next = new Float32Array(len);
    this[name] = next;
    return next;
  }

  destroy(): void {
    this.geo.destroy();
    this.program.destroy();
    this.empty.destroy();
  }
}
