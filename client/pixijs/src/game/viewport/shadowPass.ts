//! The shadow pass — billboard silhouette shadows rasterized into the screen-space `zdepth_screen`
//! RT. NO-ALPHA / OPAQUE: the RT is blanked to opaque black each frame and only shadows draw,
//! carrying per covered texel G = the caster's TILE-Y depth, encoded `5 + row%251` (5..255/255).
//! The RESERVED 0..4 band means a blanked texel (G=0) reads as "no shadow" — no separate flag. R
//! and B stay 0 (R is reserved for the world-space COLD shadow in zdepth_world). The silhouette is
//! a HARD `discard` (AA off); overlapping shadows resolve last-writer-wins.
//!
//! Per caster we project the sprite's BILLBOARD silhouette from the light onto the ground. The
//! billboard is a flat quad standing tilted at the GROUND ANGLE θ (it rises north as it climbs);
//! its points — A,B at the canopy top, N the base-CENTRE, and the base corners extruded by ±DEPTH
//! (D+,D- left · C+,C- right) — each cast along the ray from the light through it onto z=0. The
//! ±depth corners project like everything else: the raised +depth face throws away from the light
//! (t>1), the sub-ground -depth face toward it (t<1), so the base thickness comes straight from the
//! projection. The result is a sheared/scaled copy of the sprite, base pinned at the trunk, top
//! thrown from the light, with a ±depth thickness at the foot so the base reads as solid.
//!
//! The far canopy is magnified far more than the near base, so the quad is a wide TRAPEZOID; a
//! plain two-triangle split affine-warps the silhouette off-centre. Instead we FAN five triangles
//! about the base-centre N: central A B N (canopy) plus, per side, a +depth and a -depth base face
//! — left N D+ A / N D- A, right N C+ B / N C- B. Symmetric about the centerline. (Still affine.)
//!
//! FACING + FLOORS: the above is the E/W (side) billboard, rooted on its E/W floor (the sprite
//! bbox's BOTTOM edge), rising north. A south/north-facing sprite instead roots on its N/S floor —
//! the bbox's LEFT-RIGHT centre (the midline) — which lies on the ground running N-S (head→feet).
//! From that centerline floor it raises only HALF the silhouette (the half away from the light:
//! light west → east/right half u nsFloor..maxX, light east → west/left half minX..nsFloor): each
//! column's E-W distance from the midline tilts out along east-west + up, so the raised outer edge
//! projects E/W. Same 5-triangle mesh; only the per-vertex projection + the floor differ (see
//! `project`/`nsProject`, `ShadowCaster.nsRoll`/`bbox`).
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
 *  flip, `bbox` — the present-pixel bounding box the shadow FLOORS derive from — and `tileDepth`,
 *  the caster's tile depth (0..1, = (128 + row%128)/255) written into R for the occlusion compare. */
export interface ShadowCaster {
  x: number;
  y: number;
  width: number;
  height: number;
  texture: Texture;
  flipX: boolean;
  /** The billboard's bottom half-thickness in WORLD px (the top has no depth): the ±depth
   *  extrusion of the base corners along the face normal, giving the foot a front/back face. */
  depth: number;
  /** The sprite's present-pixel bounding box (fractions 0..1 of the texture). The shadow FLOORS
   *  derive from it: the E/W floor = `maxY` (bottom edge, the foot the sprite stands on), the N/S
   *  floor = `(minX+maxX)/2` (left-right centre, the midline a rolled sprite pivots on). */
  bbox: { minX: number; maxX: number; minY: number; maxY: number };
  tileDepth: number;
  /** Billboard roll for the sprite's FACING. 0 = an E/W (side) sprite: the standard billboard,
   *  width east-west, leaning north. ±1 = an s/n (front/back) sprite: the card is rolled 90° about
   *  the N-S axis so it stands EDGE-ON (faces east-west) and casts its shadow E/W. The sign (s vs
   *  n) flips the roll direction — hence which trunk corner grounds and which way it leans. */
  nsRoll: number;
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
      uniform float uTileDepth;    // caster tile depth, encoded 5+row%251 (5..255/255) → G
    `,
    main: /* glsl */ `
      // NO-ALPHA: the RT is blanked to opaque black each frame and only shadows draw. HARD
      // silhouette — discard outside the sprite's coverage (AA off) so the projected quad takes the
      // billboard's SHAPE, a clean cutout. G = the caster's tile-Y depth, encoded 5 + row%251
      // (5..255/255) so the RESERVED 0..4 band (a blanked texel reads G=0) means 'no shadow' with no
      // separate flag. R and B stay 0. OPAQUE. Overlapping shadows: last writer wins.
      if (texture(uSprite, uSpriteRect.xy + vUV * uSpriteRect.zw).b < 0.5) discard;
      outColor = vec4(0.0, uTileDepth, 0.0, 1.0);
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

  /** The zdepth_screen RT (OPAQUE: G = caster tile-Y depth, 5+row%251; 0..4 = no shadow). Null
   *  before the first {@link render}/{@link clear}. */
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
      // 7 verts (A,B canopy · N base-centre · D+,D- / C+,C- the ±depth base corners), FIVE
      // triangles about N: central A B N, left N D+ A + N D- A, right N C+ B + N C- B. See render().
      const geo = new MeshGeometry({
        positions: new Float32Array(14),
        uvs: new Float32Array(14),
        indices: new Uint32Array([0, 1, 2, 2, 3, 0, 2, 4, 0, 2, 5, 1, 2, 6, 1]),
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

  /** Rasterize every caster's projected billboard-silhouette shadow for one light into the RT.
   *  `thetaRad` is the ground angle (0 = the billboard lies flat, π/2 = it stands straight up). */
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
      // Shadow FLOORS from the sprite's present-pixel bounding box: the E/W floor is the box's
      // BOTTOM edge (the foot the sprite plants on + the texture base); the N/S floor is its
      // LEFT-RIGHT CENTRE (the midline the rolled sprite pivots on + its texture base).
      const ewFloor = c.bbox.maxY;
      const nsFloor = (c.bbox.minX + c.bbox.maxX) / 2;
      const xL = c.x;
      const xR = c.x + c.width;
      const nOrigin = c.y + ewFloor * c.height; // world-y of the origin (E/W foot = bbox bottom)
      const Hspr = ewFloor * c.height; // origin → box top
      const vBot = ewFloor; // crop the below-foot padding
      const depB = c.depth; // base half-thickness (±depth extrusion)
      const uL = c.flipX ? 1 : 0;
      const uR = c.flipX ? 0 : 1;
      // Two projections: E/W casters use `project` (foot floor, rises north), N/S casters use
      // `nsProject` (centerline floor, rises east-west). Both cast a point from the light onto z=0
      // at t = Lz/(Lz−z); `Math.max(Lz−z, 1)` only guards the grazing case.
      const rs = c.nsRoll;
      // E/W (side) billboard: width along east-west (E), rooted on the FOOT floor, leaning NORTH by
      // v·cosθ and up by v·sinθ; depth pushes north/up along the face normal. (rs = 0.)
      const project = (E: number, v: number, dep: number, sign: number): [number, number] => {
        const worldY = nOrigin - v * ct + sign * dep * st;
        const z = v * st + sign * dep * ct;
        const t = Math.min(Lz / Math.max(Lz - z, 1), TMAX);
        return [toBodyX(light.x + t * (E - light.x)), toBodyY(light.y + t * (worldY - light.y))];
      };
      // N/S (front/back) shadow: ROOTED on the CENTERLINE floor, not the foot. The sprite's midline
      // (u = nsFloor) lies on the ground running N-S — head→feet (`vFrac`) maps to world-y about the
      // box centre. The half-silhouette RISES sideways from the midline: its signed E-W distance
      // from the centerline (`uFrac − nsFloor`) tilts out along east-west by cosθ and up by sinθ, so
      // the raised outer edge projects E/W. Depth extrudes east-west (worldX ± dep).
      const eCenter = c.x + nsFloor * c.width; // centerline world-x
      const nCenter = c.y + c.height / 2; // box centre-row (the midline's north-south anchor)
      const vMid = (c.bbox.minY + c.bbox.maxY) / 2; // sprite vertical centre (head↔feet midpoint)
      const nsProject = (uFrac: number, vFrac: number, dep: number, sign: number): [number, number] => {
        const off = (uFrac - nsFloor) * c.width; // signed E-W distance from the midline
        const worldX = eCenter + off * ct + sign * dep;
        const z = Math.abs(off) * st;
        const worldY = nCenter + (vFrac - vMid) * c.height;
        const t = Math.min(Lz / Math.max(Lz - z, 1), TMAX);
        return [toBodyX(light.x + t * (worldX - light.x)), toBodyY(light.y + t * (worldY - light.y))];
      };

      // Build the 7 verts + UVs. Both use the same 5-triangle mesh about the trunk centre N
      // (indices [0,1,2, 2,3,0, 2,4,0, 2,5,1, 2,6,1]).
      let ax = 0, ay = 0, bx = 0, by = 0, nx = 0, ny = 0;
      let dpx = 0, dpy = 0, dmx = 0, dmy = 0, cpx = 0, cpy = 0, cmx = 0, cmy = 0;
      let uvs: number[];
      if (rs === 0) {
        // E/W: FULL silhouette on the foot floor. A,B canopy top; N base-centre; D±/C± the ±depth
        // base corners. UVs: A,D± left edge, B,C± right edge, N centre.
        [ax, ay] = project(xL, Hspr, 0, 1);
        [bx, by] = project(xR, Hspr, 0, 1);
        [nx, ny] = project((xL + xR) / 2, 0, 0, 1);
        [dpx, dpy] = project(xL, 0, depB, 1);
        [dmx, dmy] = project(xL, 0, depB, -1);
        [cpx, cpy] = project(xR, 0, depB, 1);
        [cmx, cmy] = project(xR, 0, depB, -1);
        uvs = [uL, 0, uR, 0, (uL + uR) / 2, vBot, uL, vBot, uL, vBot, uR, vBot, uR, vBot];
      } else {
        // N/S: HALF silhouette hinged on the CENTERLINE floor. Keep the half AWAY from the light —
        // light WEST → east/right half (u nsFloor..maxX), light EAST → west/left half (minX..nsFloor)
        // — from the midline (on the ground) out to the far bbox edge (raised). Head = bbox top
        // (minY), feet = bbox bottom (maxY). Inner verts (B head, N feet) sit ON the centerline
        // floor; outer verts (A head, D± feet ±depth) are raised. C± = D± (one outer edge).
        const uEdge = light.x < eCenter ? c.bbox.maxX : c.bbox.minX;
        const vHead = c.bbox.minY;
        const vFeet = c.bbox.maxY;
        [ax, ay] = nsProject(uEdge, vHead, 0, 1); // outer head
        [bx, by] = nsProject(nsFloor, vHead, 0, 1); // centerline head
        [nx, ny] = nsProject(nsFloor, vFeet, 0, 1); // centerline feet
        [dpx, dpy] = nsProject(uEdge, vFeet, depB, 1); // outer feet +depth
        [dmx, dmy] = nsProject(uEdge, vFeet, depB, -1); // outer feet -depth
        cpx = dpx; cpy = dpy; cmx = dmx; cmy = dmy;
        uvs = [uEdge, vHead, nsFloor, vHead, nsFloor, vFeet, uEdge, vFeet, uEdge, vFeet, uEdge, vFeet, uEdge, vFeet];
      }
      const s = this.slot(n++);
      s.pos.set([ax, ay, bx, by, nx, ny, dpx, dpy, dmx, dmy, cpx, cpy, cmx, cmy]);
      s.uv.set(uvs);
      s.posBuf.update();
      s.uvBuf.update();
      s.shader.texture = c.texture;
      s.shader.tileDepth = c.tileDepth;
      this.container.addChild(s.mesh);
    }
    renderer.render({ container: this.container, target: this.rt!, clear: true, clearColor: [0, 0, 0, 1] }); // opaque blank
  }

  /** Blank the RT to opaque black (no casters / no shadow light this frame). */
  clear(renderer: Renderer, w: number, h: number, res: number): void {
    this.ensureRT(renderer, w, h, res);
    this.container.removeChildren();
    renderer.render({ container: this.container, target: this.rt!, clear: true, clearColor: [0, 0, 0, 1] });
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
