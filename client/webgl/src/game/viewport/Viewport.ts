//! The viewport renderer (webgl) — owns its **own** `<canvas>` + engine {@link Renderer}
//! (F6: the viewport is the one WebGL surface; it self-canvases in a DOM panel body) and
//! the {@link Camera}. W4a is the host + camera slice: each frame it clears and draws a
//! procedural world-grid (tiles/zones/regions) so panning + zooming are visible before any
//! world content exists. The SquareCache G-buffer + albedo display + shadows land on this
//! same `Renderer` in W4c/W4d (see docs/work/webgl-engine/todo.md).

import { Renderer, Program, Geometry } from "../../gl";
import { Camera } from "./Camera";
import { SQUARE, ZONE_DIM, REGION_DIM } from "./squareMath";

// Fullscreen quad → its screen px are mapped to world px by the camera uniforms; grid lines
// are drawn procedurally where a world coordinate is near a multiple of a division's pitch.
const GRID_VERT = /* glsl */ `#version 300 es
in vec2 aPosition;              // clip-space fullscreen triangle-pair (-1..1)
out vec2 vClip;
void main() { vClip = aPosition; gl_Position = vec4(aPosition, 0.0, 1.0); }
`;
const GRID_FRAG = /* glsl */ `#version 300 es
precision highp float;
in vec2 vClip;
uniform vec2  uViewport;        // body size, CSS px
uniform vec2  uAnchor;          // world px at the viewport centre
uniform float uZoom;            // screen px per world px
uniform vec3  uPitch;           // tile / zone / region pitch, world px
uniform float uLevel;           // grid detail: 1 all, 2 zone+region, 3 region only
out vec4 fragColor;

// Coverage (0..1) of a grid line at world-pos w for a division of pitch world px,
// halfPx screen-px half-width. Antialiased in screen space.
float line(vec2 w, float pitch, float halfPx) {
  vec2 m = abs(mod(w, pitch));
  m = min(m, pitch - m);        // world-px distance to the nearest line, per axis
  vec2 sp = m * uZoom;          // → screen px
  float d = min(sp.x, sp.y);
  return 1.0 - smoothstep(halfPx - 1.0, halfPx + 1.0, d);
}

void main() {
  // Screen px, y DOWN (CSS convention, y=0 at top). The world is y-down — matching the
  // camera's screenToWorld — so mapping clip-space (y-up) straight through would invert the
  // grid's vertical pan. Flip y here so the displayed grid tracks the cursor on both axes.
  vec2 fragPx = vec2((vClip.x * 0.5 + 0.5) * uViewport.x, (0.5 - vClip.y * 0.5) * uViewport.y);
  vec2 world = uAnchor + (fragPx - uViewport * 0.5) / uZoom;
  vec3 col = vec3(0.0);
  float a = 0.0;
  // Fine → coarse so a coarser line wins on a shared boundary.
  if (uLevel <= 1.5) { float t = line(world, uPitch.x, 1.0) * 0.30; col = mix(col, vec3(1.0, 0.25, 0.25), t); a = max(a, t); }
  if (uLevel <= 2.5) { float z = line(world, uPitch.y, 1.5) * 0.45; col = mix(col, vec3(1.0, 0.30, 1.0), z); a = max(a, z); }
  { float r = line(world, uPitch.z, 2.0) * 0.65; col = mix(col, vec3(0.30, 0.55, 1.0), r); a = max(a, r); }
  fragColor = vec4(col, a);
}
`;

export class Viewport {
  readonly camera = new Camera();
  private readonly renderer: Renderer;
  private readonly grid: Program;
  private readonly quad: Geometry;
  /** Debug-grid detail level (0 off, 1 tile+zone+region, 2 zone+region, 3 region). */
  private gridLevel = 1;

  constructor() {
    this.renderer = new Renderer();
    this.renderer.canvas.style.cssText = "display:block;width:100%;height:100%;";
    this.grid = new Program(this.renderer.gl, GRID_VERT, GRID_FRAG, "viewport-grid");
    this.quad = new Geometry(this.renderer.gl, this.grid, {
      aPosition: { data: new Float32Array([-1, -1, 3, -1, -1, 3]), size: 2 }, // one big triangle covers the screen
    });
  }

  /** The canvas the DOM panel body hosts + input listeners attach to. */
  get canvas(): HTMLCanvasElement {
    return this.renderer.canvas;
  }

  /** Size the camera to the body rect (CSS px). The canvas fills the body via CSS; the
   *  drawing buffer is matched to the DPR each frame in {@link tick}. */
  setBounds(width: number, height: number): void {
    this.camera.setBounds(width, height);
  }

  setDebugGrid(level: number): void {
    this.gridLevel = level;
  }
  get debugGrid(): number {
    return this.gridLevel;
  }

  screenToWorld(sx: number, sy: number): { x: number; y: number } {
    return this.camera.screenToWorld(sx, sy);
  }
  worldToScreen(wx: number, wy: number): { x: number; y: number } {
    return this.camera.worldToScreen(wx, wy);
  }

  /** Per-frame: match the drawing buffer to the body, clear, draw the grid. */
  tick(): void {
    this.renderer.resize();
    // Camera bounds follow the CSS body size (DPR handled by the renderer).
    const w = this.renderer.canvas.clientWidth;
    const h = this.renderer.canvas.clientHeight;
    if (w <= 0 || h <= 0) return;
    this.camera.setBounds(w, h);
    this.renderer.clearScreen(0.05, 0.06, 0.08, 1.0);
    if (this.gridLevel <= 0) return;
    this.renderer.draw({
      program: this.grid,
      geometry: this.quad,
      blend: "normal",
      uniforms: (p) => {
        p.uVec2("uViewport", w, h);
        p.uVec2("uAnchor", this.camera.anchorX, this.camera.anchorY);
        p.uFloat("uZoom", this.camera.zoom);
        p.uVec3("uPitch", SQUARE, SQUARE * ZONE_DIM, SQUARE * ZONE_DIM * REGION_DIM);
        p.uFloat("uLevel", this.gridLevel);
      },
    });
  }

  destroy(): void {
    this.quad.destroy();
    this.grid.destroy();
  }
}
