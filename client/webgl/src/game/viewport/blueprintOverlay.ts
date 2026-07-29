//! The blueprint preview overlay (build-walls P2, D4) — CLIENT-SIDE ONLY: while the user
//! drags a wall rectangle, this draws the rect's perimeter tiles with the BLUEPRINT linked
//! atlas (variant-aware against the preview shape itself — the same D1 formula the world
//! uses), semi-transparent, over the lit world. It is a dedicated overlay pass (fork F2),
//! NOT cold-cache prims: no bake churn while the rect resizes every frame, and clearing is
//! dropping the list. Tiles whose atlas hasn't streamed yet draw a flat translucent blue.

import { Renderer, Program, Geometry, Texture } from "../../gl";
import type { Camera } from "./Camera";
import { SQUARE } from "./squareMath";

/** One previewed tile: world tile coords + the blueprint cell's atlas frame (px rect on its
 *  page), or null while unstreamed (flat fallback fill). */
export interface BlueprintTile {
  tileX: number;
  tileY: number;
  frame: { source: Texture; x: number; y: number; w: number; h: number } | null;
}

const VERT = /* glsl */ `#version 300 es
layout(location = 0) in vec2 aUnit;
uniform mat3 uProjection;
uniform vec4 uRect;                 // world px x,y,w,h
out vec2 vUnit;
void main() {
  vUnit = aUnit;
  vec3 clip = uProjection * vec3(uRect.xy + aUnit * uRect.zw, 1.0);
  gl_Position = vec4(clip.xy, 0.0, 1.0);
}`;

const FRAG = /* glsl */ `#version 300 es
precision highp float;
uniform sampler2D uTex;
uniform vec4 uFrame;                // atlas px rect (x,y,w,h)
uniform int uHasTex;
in vec2 vUnit;
out vec4 fragColor;
void main() {
  if (uHasTex == 0) {
    fragColor = vec4(0.25, 0.5, 1.0, 0.30);   // unstreamed: flat translucent blueprint blue
    return;
  }
  vec4 c = texelFetch(uTex, ivec2(uFrame.xy + vUnit * uFrame.zw), 0);
  if (c.a < 0.05) discard;
  fragColor = vec4(c.rgb, c.a * 0.65);
}`;

export class BlueprintOverlay {
  private readonly prog: Program;
  private readonly quad: Geometry;
  private tiles: BlueprintTile[] = [];

  constructor(private readonly renderer: Renderer, private readonly empty: Texture) {
    this.prog = new Program(renderer.gl, VERT, FRAG, "blueprint-preview");
    this.quad = new Geometry(renderer.gl, this.prog, {
      aUnit: { data: new Float32Array([0, 0, 1, 0, 1, 1, 0, 1]), size: 2 },
    }, new Uint32Array([0, 1, 2, 0, 2, 3]));
  }

  /** Replace the previewed set (empty = nothing drawn). The scene rebuilds it per drag move. */
  set(tiles: BlueprintTile[]): void {
    this.tiles = tiles;
  }

  draw(camera: Camera): void {
    if (this.tiles.length === 0) return;
    const w = camera.width, h = camera.height, rs = camera.renderScale;
    const proj = new Float32Array([(2 * rs) / w, 0, 0, 0, -(2 * rs) / h, 0,
                                   (-camera.anchorX * 2 * rs) / w, (camera.anchorY * 2 * rs) / h, 1]);
    for (const t of this.tiles) {
      const f = t.frame;
      this.renderer.draw({
        program: this.prog,
        geometry: this.quad,
        blend: "normal",
        textures: { uTex: f?.source ?? this.empty },
        uniforms: (p) => {
          p.uMat3("uProjection", proj);
          p.uVec4("uRect", t.tileX * SQUARE, t.tileY * SQUARE, SQUARE, SQUARE);
          p.uVec4("uFrame", f?.x ?? 0, f?.y ?? 0, f?.w ?? 1, f?.h ?? 1);
          p.uInt("uHasTex", f ? 1 : 0);
        },
      });
    }
  }
}
