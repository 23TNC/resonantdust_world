//! The blueprint preview overlay (build-walls P2, D4) — CLIENT-SIDE ONLY: while the user
//! drags a wall rectangle, this draws the rect's perimeter tiles with the BLUEPRINT linked
//! atlas (variant-aware against the preview shape itself — the same D1 formula the world
//! uses), semi-transparent, over the lit world. It is a dedicated overlay pass (fork F2),
//! NOT cold-cache prims: no bake churn while the rect resizes every frame, and clearing is
//! dropping the list. Tiles whose atlas hasn't streamed yet draw a flat translucent blue.
//!
//! texture-generalization P4 (tile-lighting F3): the preview is LIT — it samples the live
//! cold + hot lightmaps (the ephemeral path: read-only, nothing baked, the accumulators are
//! untouched) plus the ambient floor, so a blueprint dragged beside a torch glows with it.

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

/** The live-lighting inputs for the preview — the SAME values the blit samples with. Null
 *  while the lightmap isn't ready (the preview draws unlit, exactly as before). */
export interface BlueprintLighting {
  coldLight: Texture;
  hotLight: Texture;
  quant: number;
  ambient: number;
  cols: number;
  rows: number;
  winCol: number;
  winRow: number;
  lslot: number;
}

const VERT = /* glsl */ `#version 300 es
layout(location = 0) in vec2 aUnit;
uniform mat3 uProjection;
uniform vec4 uRect;                 // world px x,y,w,h
out vec2 vUnit;
out vec2 vWorld;
void main() {
  vUnit = aUnit;
  vWorld = uRect.xy + aUnit * uRect.zw;
  vec3 clip = uProjection * vec3(vWorld, 1.0);
  gl_Position = vec4(clip.xy, 0.0, 1.0);
}`;

const FRAG = /* glsl */ `#version 300 es
precision highp float;
uniform sampler2D uTex;
uniform vec4 uFrame;                // atlas px rect (x,y,w,h)
uniform int uHasTex;
uniform sampler2D uColdLight;       // the live lightmaps — read-only (the ephemeral path)
uniform sampler2D uHotLight;
uniform int uLightEnable;
uniform float uLightQuant, uAmbient;
uniform int uLCols, uLRows, uLWinCol, uLWinRow, uLSlot;
in vec2 vUnit;
in vec2 vWorld;
out vec4 fragColor;
const float SQ = ${SQUARE.toFixed(1)};
int pmod(int a, int m) { return ((a % m) + m) % m; }
// World → the toroidal FINE lightmap texel — the blit's own mapping, verbatim.
ivec2 lightTexel(vec2 world) {
  int tx = int(floor(world.x / SQ)), ty = int(floor(world.y / SQ));
  if (tx < uLWinCol || tx >= uLWinCol + uLCols || ty < uLWinRow || ty >= uLWinRow + uLRows) return ivec2(-1);
  int sx = pmod(tx, uLCols), sy = pmod(ty, uLRows);
  float lx = fract(world.x / SQ), ly = fract(world.y / SQ);
  return ivec2(sx * uLSlot + int(lx * float(uLSlot)), sy * uLSlot + int(ly * float(uLSlot)));
}
void main() {
  vec3 light = vec3(1.0);
  if (uLightEnable == 1) {
    ivec2 lt = lightTexel(vWorld);
    if (lt.x >= 0) {
      vec3 irr = (texelFetch(uColdLight, lt, 0).rgb + texelFetch(uHotLight, lt, 0).rgb) / uLightQuant;
      light = uAmbient + min(max(irr, vec3(0.0)), vec3(4.0));   // the blit's display clamp
    }
  }
  if (uHasTex == 0) {
    fragColor = vec4(vec3(0.25, 0.5, 1.0) * light, 0.30);   // unstreamed: flat translucent blueprint blue, lit
    return;
  }
  vec4 c = texelFetch(uTex, ivec2(uFrame.xy + vUnit * uFrame.zw), 0);
  if (c.a < 0.05) discard;
  fragColor = vec4(c.rgb * light, c.a * 0.65);
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

  draw(camera: Camera, lighting: BlueprintLighting | null): void {
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
        textures: {
          uTex: f?.source ?? this.empty,
          uColdLight: lighting?.coldLight ?? this.empty,
          uHotLight: lighting?.hotLight ?? this.empty,
        },
        uniforms: (p) => {
          p.uMat3("uProjection", proj);
          p.uVec4("uRect", t.tileX * SQUARE, t.tileY * SQUARE, SQUARE, SQUARE);
          p.uVec4("uFrame", f?.x ?? 0, f?.y ?? 0, f?.w ?? 1, f?.h ?? 1);
          p.uInt("uHasTex", f ? 1 : 0);
          p.uInt("uLightEnable", lighting ? 1 : 0);
          p.uFloat("uLightQuant", lighting?.quant ?? 1);
          p.uFloat("uAmbient", lighting?.ambient ?? 0);
          p.uInt("uLCols", lighting?.cols ?? 1);
          p.uInt("uLRows", lighting?.rows ?? 1);
          p.uInt("uLWinCol", lighting?.winCol ?? 0);
          p.uInt("uLWinRow", lighting?.winRow ?? 0);
          p.uInt("uLSlot", lighting?.lslot ?? 1);
        },
      });
    }
  }
}
