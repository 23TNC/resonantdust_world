//! The selection outline overlay (ui-select P1, D5) — drawn AFTER the display blit, one quad
//! per selected item, in world space through the same projection the shadow overlay uses.
//! Two modes: SPRITE samples the co-packed surface frame's B lane (silhouette coverage) and
//! lights the OUTER edge — fragments just off the silhouette with an opaque neighbor — so the
//! outline hugs the drawn art at the art's own resolution; BOX is a screen-constant border
//! ring (tile selections, and the fallback when no surface frame resolves). The fork D5
//! anticipated: the silhouette read costs 5 texel fetches per fragment over a handful of small
//! quads — proportionate, no fallback demotion needed.

import { Renderer, Program, Geometry, Texture } from "../../gl";
import type { Camera } from "./Camera";

/** One outlined selection. `frame` = the surface atlas sub-rect (silhouette in B); absent →
 *  BOX mode regardless of `mode`. */
export interface OutlineItem {
  x: number;
  y: number;
  w: number;
  h: number;
  mode: "sprite" | "box";
  frame?: { source: Texture; x: number; y: number; w: number; h: number };
  flip?: boolean;
}

const VERT = /* glsl */ `#version 300 es
layout(location = 0) in vec2 aUnit;
uniform mat3 uProjection;
uniform vec4 uRect;                  // world px x,y,w,h
out vec2 vUnit;
void main() {
  vUnit = aUnit;
  vec3 clip = uProjection * vec3(uRect.xy + aUnit * uRect.zw, 1.0);
  gl_Position = vec4(clip.xy, 0.0, 1.0);
}`;

const FRAG = /* glsl */ `#version 300 es
precision highp float;
uniform int uMode;                   // 0 sprite silhouette edge | 1 box border ring
uniform sampler2D uSurf;             // the surface atlas page (silhouette = B)
uniform vec4 uFrame;                 // frame px rect on the page (x,y,w,h)
uniform int uFlip;
uniform vec3 uColor;
uniform vec2 uRectPx;                // the quad's SCREEN px size (screen-constant border)
in vec2 vUnit;
out vec4 fragColor;
float silAt(vec2 fpx) {
  if (fpx.x < 0.0 || fpx.y < 0.0 || fpx.x >= uFrame.z || fpx.y >= uFrame.w) return 0.0;
  return texelFetch(uSurf, ivec2(uFrame.xy + fpx), 0).b;
}
void main() {
  if (uMode == 1) {
    vec2 dpx = min(vUnit, 1.0 - vUnit) * uRectPx;   // px distance to the nearest edge, per axis
    if (min(dpx.x, dpx.y) > 2.0) discard;
    fragColor = vec4(uColor, 0.9);
    return;
  }
  vec2 uv = vUnit;
  if (uFlip == 1) uv.x = 1.0 - uv.x;
  vec2 fpx = uv * uFrame.zw;
  if (silAt(fpx) >= 0.5) discard;                   // inside the sprite: keep the art
  float m = max(max(silAt(fpx + vec2(2.0, 0.0)), silAt(fpx + vec2(-2.0, 0.0))),
                max(silAt(fpx + vec2(0.0, 2.0)), silAt(fpx + vec2(0.0, -2.0))));
  if (m < 0.5) discard;                             // not adjacent to the silhouette either
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
      const f = it.frame;
      this.renderer.draw({
        program: this.prog,
        geometry: this.quad,
        blend: "normal",
        textures: { uSurf: f?.source ?? this.empty },
        uniforms: (p) => {
          p.uMat3("uProjection", proj);
          p.uVec4("uRect", it.x, it.y, it.w, it.h);
          p.uInt("uMode", it.mode === "sprite" && f ? 0 : 1);
          p.uVec4("uFrame", f?.x ?? 0, f?.y ?? 0, f?.w ?? 1, f?.h ?? 1);
          p.uInt("uFlip", it.flip ? 1 : 0);
          p.uVec3("uColor", 1.0, 0.9, 0.35);
          p.uVec2("uRectPx", it.w * rs, it.h * rs);
        },
      });
    }
  }

  destroy(): void {
    this.prog.destroy?.();
  }
}
