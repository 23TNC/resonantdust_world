//! The shadow pass — billboard "wedge" shadows rasterized into a screen-space RT that carries,
//! per covered texel: R = the caster's TILE-DEPTH (128 + row%128, /255, PREMULTIPLIED by
//! coverage) and A = coverage (the silhouette alpha).
//!
//! Per caster we build the sim's wedge: a quad (A,B top / C,D bottom) rotated about its bottom
//! edge by the GROUND ANGLE θ, with per-vertex depth extruded ± along the face normal. The TOP
//! depth is 0 (A,B share a point across faces); the BOTTOM has depth. We anchor the front-bottom
//! to the ORIGIN (trunk base) and drop the model in z so the front (blue) face sits on the
//! ground; the back (red) face's sub-ground points clamp to z=0 (their footprint). Instead of a
//! front/back face pair (which leaves the sides open), we draw two CROSSED rectangles —
//! `A B C+ D−` and `A B C− D+` — so their bottom edges run opposite diagonals and cover the
//! sides with an X. Each rectangle is textured with the sprite silhouette (its alpha).
//!
//! The OCCLUSION (don't shadow a thing that sits in front of the caster) is NOT done here — it
//! lives in the lighting pass, which compares this RT's caster depth against the world-space
//! `depth` composite (the receiver's own tile depth) at each fragment. This pass just projects.
//!
//! Coordinates: projection runs in WORLD px (E = world x; N = world y, +south/down-screen;
//! z = height), then ground points map to BODY px via the `toBody*` fns so the RT aligns 1:1
//! with the display.

import {
  compileHighShaderGlProgram,
  localUniformBitGl,
  roundPixelsBitGl,
  type Buffer,
  Container,
  GlProgram,
  Mesh,
  MeshGeometry,
  RenderTexture,
  Shader,
  Texture,
  UniformGroup,
  type Renderer,
} from "pixi.js";

/** Projection `t` cap — a light grazing a point's height would send the shadow to infinity;
 *  clamp it until the reach-clip lands. `t` = ground-distance ÷ horizontal light distance. */
const TMAX = 8;

/** A shadow caster: the sprite's billboard box in WORLD px, the silhouette texture, the west
 *  flip, the bottom half-thickness `depth` (top depth is 0), `footFrac` — the origin (trunk
 *  base) as a fraction down the box (0 = top, 1 = bottom), where the front foot plants — and
 *  `tileDepth`, the caster's tile depth (0..1, = (128 + row%128)/255) written into R for the
 *  lighting-pass occlusion compare. */
export interface ShadowCaster {
  x: number;
  y: number;
  width: number;
  height: number;
  texture: Texture;
  flipX: boolean;
  depth: number;
  footFrac: number;
  tileDepth: number;
}

/** A shadow-casting point light: world px + height above the ground. */
export interface ShadowLight {
  x: number;
  y: number;
  height: number;
}

/** Maps a world-px coordinate to body px (the on-screen viewport space the RT covers). */
export type ToBody = (world: number) => number;

// ── coverage shader ───────────────────────────────────────────────────────────────
// Samples the sprite's atlas sub-region and writes its ALPHA (silhouette) as coverage, with the
// caster's tile depth in R. Drops the stock textureBit; maps the frame ourselves via uSpriteRect.

/** A texture's atlas-page uv rect `[offsetU, offsetV, scaleU, scaleV]`. */
function uvRect(t: Texture): Float32Array {
  const f = t.frame;
  return new Float32Array([f.x / t.source.width, f.y / t.source.height, f.width / t.source.width, f.height / t.source.height]);
}

const shadowBitGl = {
  name: "shadow-coverage-bit",
  vertex: { header: "", main: "" },
  fragment: {
    header: /* glsl */ `
      uniform sampler2D uSprite;   // caster's SURFACE map (atlas source); B = silhouette/coverage
      uniform vec4 uSpriteRect;    // sprite's uv rect on its page (offset.xy, scale.zw)
      uniform float uTileDepth;    // caster tile depth 0..1 → R (premultiplied by coverage)
    `,
    main: /* glsl */ `
      // Cast a SOLID silhouette: harden surface.B to presence (same band as the depth/surface
      // bakes) so the shadow carries no foliage-alpha dapple, and the two crossed wedge
      // rectangles union cleanly — near-binary coverage leaves no partial-overlap seam specks.
      float a = smoothstep(0.35, 0.65, texture(uSprite, uSpriteRect.xy + vUV * uSpriteRect.zw).b);
      outColor = vec4(uTileDepth * a, 0.0, 0.0, a);   // R = depth·cov, A = cov; over-blend unions
    `,
  },
};

let program: GlProgram | null = null;
function shadowProgram(): GlProgram {
  if (!program) {
    program = compileHighShaderGlProgram({ name: "shadow-coverage", bits: [localUniformBitGl, shadowBitGl, roundPixelsBitGl] });
  }
  return program;
}

/** One rectangle's coverage material — its own uniform group so pooled instances hold their own
 *  sprite binding, letting the whole pass render in one `renderer.render`. */
class ShadowShader extends Shader {
  private _tex: Texture = Texture.EMPTY;
  get texture(): Texture {
    return this._tex;
  }
  set texture(value: Texture) {
    this._tex = value;
    this.resources.uSprite = value.source;
    this.resources.uSpriteSampler = value.source.style;
    this.resources.shadowUniforms.uniforms.uSpriteRect = uvRect(value);
    this.resources.shadowUniforms.update();
  }
  /** The caster's tile depth (0..1) written into R (premultiplied by coverage). */
  set tileDepth(value: number) {
    this.resources.shadowUniforms.uniforms.uTileDepth = value;
    this.resources.shadowUniforms.update();
  }
}

function makeShadowShader(): ShadowShader {
  const e = Texture.EMPTY;
  return new ShadowShader({
    glProgram: shadowProgram(),
    resources: {
      uSprite: e.source,
      uSpriteSampler: e.source.style,
      shadowUniforms: new UniformGroup({
        uSpriteRect: { value: new Float32Array([0, 0, 1, 1]), type: "vec4<f32>" },
        uTileDepth: { value: 1, type: "f32" },
      }),
    },
  });
}

interface Slot {
  mesh: Mesh<MeshGeometry>;
  shader: ShadowShader;
  posBuf: Buffer;
  uvBuf: Buffer;
  pos: Float32Array;
  uv: Float32Array;
}

export class ShadowPass {
  private rt: RenderTexture | null = null;
  private w = 0;
  private h = 0;
  private res = 1;
  private readonly container = new Container();
  private readonly pool: Slot[] = [];

  /** The shadow RT (R = caster depth·coverage, A = coverage). Null before the first
   *  {@link render}/{@link clear}. */
  get texture(): RenderTexture | null {
    return this.rt;
  }

  private ensureRT(renderer: Renderer, w: number, h: number, res: number): void {
    if (this.rt && this.w === w && this.h === h && this.res === res) return;
    this.rt?.destroy(true);
    this.rt = RenderTexture.create({ width: w, height: h, resolution: res });
    this.w = w;
    this.h = h;
    this.res = res;
  }

  private slot(i: number): Slot {
    let s = this.pool[i];
    if (!s) {
      const geo = new MeshGeometry({
        positions: new Float32Array(8),
        uvs: new Float32Array(8),
        indices: new Uint32Array([0, 1, 2, 0, 2, 3]),
      });
      const shader = makeShadowShader();
      const mesh = new Mesh<MeshGeometry>({ geometry: geo, shader });
      const posBuf = geo.getBuffer("aPosition");
      const uvBuf = geo.getBuffer("aUV");
      s = { mesh, shader, posBuf, uvBuf, pos: posBuf.data as Float32Array, uv: uvBuf.data as Float32Array };
      this.pool[i] = s;
    }
    return s;
  }

  /** Rasterize every caster's crossed wedge shadow for one light into the RT.
   *  `thetaRad` is the ground angle (0 = flat / depth vertical, π/2 = standing / depth N–S). */
  render(
    renderer: Renderer,
    w: number,
    h: number,
    res: number,
    casters: readonly ShadowCaster[],
    light: ShadowLight,
    thetaRad: number,
    toBodyX: ToBody,
    toBodyY: ToBody,
  ): void {
    this.ensureRT(renderer, w, h, res);
    this.container.removeChildren();
    const Lz = light.height;
    const ct = Math.cos(thetaRad);
    const st = Math.sin(thetaRad);
    let n = 0;
    for (const c of casters) {
      const xL = c.x;
      const xR = c.x + c.width;
      const nOrigin = c.y + c.footFrac * c.height; // world-y of the origin (trunk base)
      const Hspr = c.footFrac * c.height; // origin → box top
      const vBot = c.footFrac; // crop the below-origin padding
      const depB = c.depth;
      const uL = c.flipX ? 1 : 0;
      const uR = c.flipX ? 0 : 1;
      // Straddle the origin symmetrically (no shift): the depth+ bottom rises above ground and
      // the depth− bottom drops below (clamped to its footprint), so the CROSSING of the X sits
      // right on the origin (trunk base) at z=0.
      const project = (E: number, v: number, dep: number, sign: number): [number, number] => {
        const worldY = nOrigin - v * ct + sign * dep * st;
        const z = v * st + sign * dep * ct;
        const zc = Math.max(z, 0); // sub-ground → footprint, not through the floor
        const t = zc <= 0 ? 1 : Math.min(Lz / Math.max(Lz - zc, 1), TMAX);
        return [toBodyX(light.x + t * (E - light.x)), toBodyY(light.y + t * (worldY - light.y))];
      };

      // Shared top edge A, B (depth 0). Bottom corners at ±depth per side.
      const [ax, ay] = project(xL, Hspr, 0, 1);
      const [bx2, by2] = project(xR, Hspr, 0, 1);
      const [cpx, cpy] = project(xR, 0, depB, 1); // C+
      const [cmx, cmy] = project(xR, 0, depB, -1); // C−
      const [dpx, dpy] = project(xL, 0, depB, 1); // D+
      const [dmx, dmy] = project(xL, 0, depB, -1); // D−
      // Two CROSSED, textured rectangles: A B C+ D− · A B C− D+. Same silhouette UVs; only the
      // bottom positions cross. TL, TR, BR, BL order; v runs 0 (box top) → footFrac (origin).
      const uvs = [uL, 0, uR, 0, uR, vBot, uL, vBot];
      const r1 = this.slot(n++);
      r1.pos.set([ax, ay, bx2, by2, cpx, cpy, dmx, dmy]);
      r1.uv.set(uvs);
      r1.posBuf.update();
      r1.uvBuf.update();
      r1.shader.texture = c.texture;
      r1.shader.tileDepth = c.tileDepth;
      this.container.addChild(r1.mesh);
      const r2 = this.slot(n++);
      r2.pos.set([ax, ay, bx2, by2, cmx, cmy, dpx, dpy]);
      r2.uv.set(uvs);
      r2.posBuf.update();
      r2.uvBuf.update();
      r2.shader.texture = c.texture;
      r2.shader.tileDepth = c.tileDepth;
      this.container.addChild(r2.mesh);
    }
    renderer.render({ container: this.container, target: this.rt!, clear: true, clearColor: [0, 0, 0, 0] });
  }

  /** Clear the RT to zero (no casters / no shadow light this frame). */
  clear(renderer: Renderer, w: number, h: number, res: number): void {
    this.ensureRT(renderer, w, h, res);
    this.container.removeChildren();
    renderer.render({ container: this.container, target: this.rt!, clear: true, clearColor: [0, 0, 0, 0] });
  }

  destroy(): void {
    this.rt?.destroy(true);
    for (const s of this.pool) {
      s.mesh.destroy();
      s.shader.destroy();
    }
    this.container.destroy();
  }
}
