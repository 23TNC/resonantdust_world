//! The selection outline overlay (ui-select P1, D5; reworked by bug-sweep F4) — drawn AFTER
//! the display blit, one quad per selected OBJECT, in world space through the same projection
//! the shadow overlay uses. Two modes: SPRITE outlines the UNION silhouette of every part the
//! object draws (≤4 — the primitive-graph law): a fragment lights only when it is OUTSIDE all
//! part silhouettes and ADJACENT to at least one — so an edge interior to the object (the
//! body's shoulder line under the head) never draws, which is the user's neck test. BOX is a
//! screen-constant border ring (tile selections, and the fallback when no surface resolves).
//! The silhouette read is B-lane texel fetches per part over a handful of small quads —
//! proportionate (D5's costing, ×parts).

import { Renderer, Program, Geometry, Texture } from "../../gl";
import type { Camera } from "./Camera";

/** One drawn PART of an outlined object: its world quad + surface frame (silhouette in B). */
export interface OutlinePart {
  x: number;
  y: number;
  w: number;
  h: number;
  frame: { source: Texture; x: number; y: number; w: number; h: number };
  flip?: boolean;
}

/** One outlined selection. Sprite mode outlines the UNION of `parts` (1..4); an empty /
 *  absent parts list falls back to BOX mode regardless of `mode`. The item rect is the
 *  rasterized quad — the union bbox padded by the ring width. */
export interface OutlineItem {
  x: number;
  y: number;
  w: number;
  h: number;
  mode: "sprite" | "box";
  parts?: OutlinePart[];
}

const MAX_PARTS = 4;

const VERT = /* glsl */ `#version 300 es
layout(location = 0) in vec2 aUnit;
uniform mat3 uProjection;
uniform vec4 uRect;                  // world px x,y,w,h (the union bbox quad)
out vec2 vUnit;
out vec2 vWorld;
void main() {
  vUnit = aUnit;
  vWorld = uRect.xy + aUnit * uRect.zw;
  vec3 clip = uProjection * vec3(vWorld, 1.0);
  gl_Position = vec4(clip.xy, 0.0, 1.0);
}`;

// ES 3.00: sampler array subscripts must be constant — four named samplers with an
// if-chain keeps every fetch constant-indexed.
const FRAG = /* glsl */ `#version 300 es
precision highp float;
uniform int uMode;                   // 0 sprite union edge | 1 box border ring
uniform int uCount;                  // live parts (sprite mode)
uniform sampler2D uSurf0;
uniform sampler2D uSurf1;
uniform sampler2D uSurf2;
uniform sampler2D uSurf3;
uniform vec4 uPartRect[4];           // world px x,y,w,h per part
uniform vec4 uFrame[4];              // surface-page px rect per part
uniform int uFlip[4];
uniform vec3 uColor;
uniform vec2 uRectPx;                // the quad's SCREEN px size (box mode's border)
in vec2 vUnit;
in vec2 vWorld;
out vec4 fragColor;

float fetchB(int i, ivec2 px) {
  if (i == 0) return texelFetch(uSurf0, px, 0).b;
  if (i == 1) return texelFetch(uSurf1, px, 0).b;
  if (i == 2) return texelFetch(uSurf2, px, 0).b;
  return texelFetch(uSurf3, px, 0).b;
}

/* Part i's silhouette coverage at world point p (0 outside its quad). */
float silAt(int i, vec2 p) {
  vec4 r = uPartRect[i];
  vec2 uv = (p - r.xy) / r.zw;
  if (uv.x < 0.0 || uv.y < 0.0 || uv.x >= 1.0 || uv.y >= 1.0) return 0.0;
  if (uFlip[i] == 1) uv.x = 1.0 - uv.x;
  vec2 fpx = uv * uFrame[i].zw;
  return fetchB(i, ivec2(uFrame[i].xy + fpx));
}

/* The UNION coverage at p over the live parts. */
float unionAt(vec2 p) {
  float m = 0.0;
  for (int i = 0; i < 4; i++) {
    if (i >= uCount) break;
    m = max(m, silAt(i, p));
  }
  return m;
}

void main() {
  if (uMode == 1) {
    vec2 dpx = min(vUnit, 1.0 - vUnit) * uRectPx;   // px distance to the nearest edge, per axis
    if (min(dpx.x, dpx.y) > 2.0) discard;
    fragColor = vec4(uColor, 0.9);
    return;
  }
  // Inside ANY part's art: keep the art — this is what occludes interior edges (F4).
  if (unionAt(vWorld) >= 0.5) discard;
  // Adjacent to the union? Probe each part at ITS art resolution (2 art texels in world px).
  float m = 0.0;
  for (int i = 0; i < 4; i++) {
    if (i >= uCount) break;
    float s = 2.0 * uPartRect[i].z / uFrame[i].z;   // 2 art texels, world px
    m = max(m, max(max(silAt(i, vWorld + vec2(s, 0.0)), silAt(i, vWorld + vec2(-s, 0.0))),
                   max(silAt(i, vWorld + vec2(0.0, s)), silAt(i, vWorld + vec2(0.0, -s)))));
  }
  if (m < 0.5) discard;                             // not adjacent to the union either
  fragColor = vec4(uColor, 0.9);
}`;

export class OutlineOverlay {
  private readonly prog: Program;
  private readonly quad: Geometry;
  private items: OutlineItem[] = [];

  constructor(private readonly renderer: Renderer, private readonly empty: Texture) {
    this.prog = new Program(renderer.gl, VERT, FRAG, "selection-outline");
    this.quad = new Geometry(renderer.gl, this.prog, {
      aUnit: { data: new Float32Array([0, 0, 1, 0, 1, 1, 0, 1]), size: 2 },
    }, new Uint32Array([0, 1, 2, 0, 2, 3]));
  }

  /** Replace the outlined set (the scene rebuilds it per frame — movers move). */
  set(items: OutlineItem[]): void {
    this.items = items;
  }

  draw(camera: Camera): void {
    if (this.items.length === 0) return;
    const w = camera.width, h = camera.height, rs = camera.renderScale;
    const proj = new Float32Array([(2 * rs) / w, 0, 0, 0, -(2 * rs) / h, 0,
                                   (-camera.anchorX * 2 * rs) / w, (camera.anchorY * 2 * rs) / h, 1]);
    for (const it of this.items) {
      const parts = (it.parts ?? []).slice(0, MAX_PARTS);
      const sprite = it.mode === "sprite" && parts.length > 0;
      const partRect = new Float32Array(16);
      const frame = new Float32Array(16);
      const flips: number[] = [0, 0, 0, 0];
      const texs: Record<string, Texture> = {};
      for (let i = 0; i < MAX_PARTS; i++) {
        const p = parts[i];
        texs[`uSurf${i}`] = p?.frame.source ?? this.empty;
        if (!p) continue;
        partRect.set([p.x, p.y, p.w, p.h], i * 4);
        frame.set([p.frame.x, p.frame.y, p.frame.w, p.frame.h], i * 4);
        flips[i] = p.flip ? 1 : 0;
      }
      this.renderer.draw({
        program: this.prog,
        geometry: this.quad,
        blend: "normal",
        textures: texs,
        uniforms: (pr) => {
          pr.uMat3("uProjection", proj);
          pr.uVec4("uRect", it.x, it.y, it.w, it.h);
          pr.uInt("uMode", sprite ? 0 : 1);
          pr.uInt("uCount", parts.length);
          pr.uVec4Array("uPartRect", partRect);
          pr.uVec4Array("uFrame", frame);
          pr.uIntArray("uFlip", new Int32Array(flips));
          pr.uVec3("uColor", 1.0, 0.9, 0.35);
          pr.uVec2("uRectPx", it.w * rs, it.h * rs);
        },
      });
    }
  }

  destroy(): void {
    this.prog.destroy?.();
  }
}
