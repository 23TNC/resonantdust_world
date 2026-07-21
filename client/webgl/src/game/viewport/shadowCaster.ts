//! shadowCaster (webgl) — the FIRST lights + billboard shadows on the owned engine. Basic-functionality
//! cut of the `docs/work/shadows/` design: a small light list seeded around a zone, casting billboard-quad
//! shadows off the standing prims (things, `zIndex ≥ 1`) — geo tier, no textures yet.
//!
//! This cut does the cast **analytically in ONE screen-space fullscreen pass** (the design's GPU-cast
//! endpoint, caster-lut C5), deferring the screen-hot→world-cold bitfield persistence + round-robin (the
//! scaling optimisations) to a later slice. Per fragment we compute its world position (same mapping as the
//! grid shader), then for each light test whether the pixel falls inside any of that light's casters' shadow
//! trapezoids; each covering light adds its colour (overlaps combine — additive), matching
//! `design/shadows.md`'s projection. No RTs, no ping-pong: because it recasts every frame at the current
//! camera it's automatically world-stuck + zoom-correct with no staleness (the README's `shadow-hot`
//! insight). Casters are culled to those in range of a light on the CPU so the per-pixel loop stays short.
//!
//! GOTCHA: a backtick inside the GLSL closes the `/* glsl */` literal.

import { Renderer, Program, Geometry } from "../../gl";
import type { Camera } from "./Camera";
import type { Primitive } from "./SquareCache";
import { SQUARE } from "./squareMath";

/** Max lights + casters the shader loops (uniform-array sizes). 6 lights this iteration (→ 24 goal);
 *  casters are the in-range standing prims, capped so a dense zone can't blow the per-pixel loop. */
const MAX_LIGHTS = 6;
const MAX_CASTERS = 48;
/** Shadow length clamp: a caster as tall as the light casts to infinity; clamp the projection factor. */
const TMAX = 3.0;
/** Default light ring: N lights on a ~2-tile radius around the seed tile, lit from above (world z). */
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
in vec2 aPosition;                 // fullscreen triangle in clip space
out vec2 vClip;
void main() { vClip = aPosition; gl_Position = vec4(aPosition, 0.0, 1.0); }
`;
const CAST_FRAG = /* glsl */ `#version 300 es
precision highp float;
in vec2 vClip;
uniform vec2  uView;               // viewport px
uniform vec2  uAnchor;             // camera anchor (world px at viewport centre)
uniform float uZoom;               // screen px per world px
uniform int   uLightCount;
uniform vec4  uLight[${MAX_LIGHTS}];   // xyz = world pos (+z height), w = radius
uniform int   uCasterCount;
uniform vec4  uCaster[${MAX_CASTERS}]; // x,y = world top-left, z,w = width,height
out vec4 fragColor;

// Six distinct per-light colours (the design's bit-decode palette, extended to 6).
vec3 lightColor(int k) {
  if (k == 0) return vec3(1.0, 0.25, 0.25);
  if (k == 1) return vec3(0.25, 1.0, 0.30);
  if (k == 2) return vec3(0.30, 0.55, 1.0);
  if (k == 3) return vec3(1.0, 0.95, 0.25);
  if (k == 4) return vec3(1.0, 0.35, 1.0);
  return vec3(0.30, 1.0, 1.0);
}

float cross2(vec2 u, vec2 v) { return u.x * v.y - u.y * v.x; }

// Point inside a convex quad (either winding) — all edge cross-products share a sign.
bool inQuad(vec2 p, vec2 a, vec2 b, vec2 c, vec2 d) {
  float s1 = cross2(b - a, p - a);
  float s2 = cross2(c - b, p - b);
  float s3 = cross2(d - c, p - c);
  float s4 = cross2(a - d, p - d);
  bool neg = (s1 < 0.0) || (s2 < 0.0) || (s3 < 0.0) || (s4 < 0.0);
  bool pos = (s1 > 0.0) || (s2 > 0.0) || (s3 > 0.0) || (s4 > 0.0);
  return !(neg && pos);
}

void main() {
  // Fragment → world px (same mapping as the debug grid, so shadows register with the world).
  vec2 fragPx = vec2((vClip.x * 0.5 + 0.5) * uView.x, (0.5 - vClip.y * 0.5) * uView.y);
  vec2 w = uAnchor + (fragPx - uView * 0.5) / uZoom;

  vec3 acc = vec3(0.0);
  for (int k = 0; k < ${MAX_LIGHTS}; k++) {
    if (k >= uLightCount) break;
    vec3 L = uLight[k].xyz;
    float R = uLight[k].w;
    bool covered = false;
    for (int i = 0; i < ${MAX_CASTERS}; i++) {
      if (i >= uCasterCount) break;
      vec4 C = uCaster[i];
      float X = C.x, Y = C.y, W = C.z, H = C.w;
      vec2 base = vec2(X + W * 0.5, Y + H);       // caster footprint base (bottom-centre)
      if (distance(base, L.xy) > R) continue;      // out of this light's reach
      // Project the billboard's bottom edge radially away from the light to the ground.
      float factor = (L.z <= H + 1.0) ? ${TMAX.toFixed(1)} : min(L.z / (L.z - H), ${TMAX.toFixed(1)});
      vec2 bL = vec2(X, Y + H);
      vec2 bR = vec2(X + W, Y + H);
      vec2 tL = L.xy + factor * (bL - L.xy);
      vec2 tR = L.xy + factor * (bR - L.xy);
      if (inQuad(w, bL, bR, tR, tL)) { covered = true; break; }
    }
    if (covered) acc += lightColor(k);
  }

  // Light markers: a filled dot + a faint reach ring, so the light positions read against the shadows.
  for (int k = 0; k < ${MAX_LIGHTS}; k++) {
    if (k >= uLightCount) break;
    vec2 ls = (uLight[k].xy - uAnchor) * uZoom + uView * 0.5;
    float d = distance(fragPx, ls);
    if (d < 6.0) { fragColor = vec4(lightColor(k), 1.0); return; }
    float ring = uLight[k].w * uZoom;
    if (abs(d - ring) < 1.0) acc += lightColor(k) * 0.4;
  }

  if (dot(acc, acc) < 0.0001) discard;             // no shadow here — the world shows through
  fragColor = vec4(clamp(acc, 0.0, 1.0), 1.0);     // premultiplied (α=1) coloured shadow over the world
}
`;

/**
 * Owns the light list + the analytic screen-space shadow pass. The {@link Viewport} calls {@link tick}
 * each frame after the world display; casters come from the cold cache's {@link Primitive} standing prims.
 */
export class ShadowCaster {
  private readonly program: Program;
  private readonly quad: Geometry;
  private readonly lights: Light[] = [];
  private readonly lightBuf = new Float32Array(MAX_LIGHTS * 4);
  private readonly casterBuf = new Float32Array(MAX_CASTERS * 4);
  private enabled = true;

  constructor(private readonly renderer: Renderer) {
    const gl = renderer.gl;
    this.program = new Program(gl, CAST_VERT, CAST_FRAG, "viewport-shadow-cast");
    this.quad = new Geometry(gl, this.program, {
      aPosition: { data: new Float32Array([-1, -1, 3, -1, -1, 3]), size: 2 },
    });
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

  /** Draw the shadows over the world display. `standing` = the cold cache's standing prims (things). */
  tick(camera: Camera, standing: Primitive[]): void {
    if (!this.enabled || this.lights.length === 0) return;

    // Lights → uniform buffer.
    const nLights = Math.min(this.lights.length, MAX_LIGHTS);
    for (let k = 0; k < nLights; k++) {
      const L = this.lights[k];
      this.lightBuf[k * 4] = L.x;
      this.lightBuf[k * 4 + 1] = L.y;
      this.lightBuf[k * 4 + 2] = L.z;
      this.lightBuf[k * 4 + 3] = L.radius;
    }

    // Cull casters to those in range of ANY light (keeps the per-pixel loop short), cap at MAX_CASTERS.
    let n = 0;
    for (const p of standing) {
      if (n >= MAX_CASTERS) break;
      const bx = p.x + p.width * 0.5;
      const by = p.y + p.height;
      let near = false;
      for (let k = 0; k < nLights; k++) {
        const L = this.lights[k];
        if (Math.hypot(bx - L.x, by - L.y) <= L.radius) { near = true; break; }
      }
      if (!near) continue;
      this.casterBuf[n * 4] = p.x;
      this.casterBuf[n * 4 + 1] = p.y;
      this.casterBuf[n * 4 + 2] = p.width;
      this.casterBuf[n * 4 + 3] = p.height;
      n++;
    }

    const w = camera.width;
    const h = camera.height;
    this.renderer.draw({
      program: this.program,
      geometry: this.quad,
      blend: "normal",
      uniforms: (prog) => {
        prog.uVec2("uView", w, h);
        prog.uVec2("uAnchor", camera.anchorX, camera.anchorY);
        prog.uFloat("uZoom", camera.zoom);
        prog.uInt("uLightCount", nLights);
        prog.uVec4Array("uLight", this.lightBuf);
        prog.uInt("uCasterCount", n);
        prog.uVec4Array("uCaster", this.casterBuf);
      },
    });
  }

  destroy(): void {
    this.quad.destroy();
    this.program.destroy();
  }
}
