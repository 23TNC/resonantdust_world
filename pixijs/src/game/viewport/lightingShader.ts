//! The viewport's deferred lighting shader — the display mesh's swap-in for Pixi's default
//! flat-texture material. It samples the toroidal composite's NORMAL and ALBEDO channels
//! (both at the same per-vertex `aUV`) and lights per-fragment:
//!
//!   lit = albedo.rgb × (ambient + sunColor · halfLambert(N · sunDir))
//!
//! Adapted from the old game's `GroundShader` (`../resonantdust/view/.../rectDisplayShader.ts`)
//! but collapsed to the new game's single-composite model: there are no per-chunk lit bakes
//! and no chunk-local light coords — the composites already form a screen-space G-buffer, so
//! this is one screen-space pass. Built on the Pixi v8 high-shader bits so it drops onto the
//! existing `Mesh` (`aPosition`/`aUV`) with the projection UBO and pixel-snap bound for free.
//!
//! Phase B = ambient + a single directional SUN. Point lights (Phase C) and depth-based
//! shadows (Phase D) extend `lightBitGl`'s fragment `main` and the uniform group in place.
//!
//! GOTCHAS (silent failure modes — see the plan): a GLSL compile error makes the mesh draw
//! BLACK with only a `console.error`; `packed` is a reserved word; a backtick inside a GLSL
//! comment closes the template literal.

import {
  compileHighShaderGlProgram,
  localUniformBitGl,
  textureBitGl,
  roundPixelsBitGl,
  GlProgram,
  Shader,
  Texture,
  Matrix,
  UniformGroup,
} from "pixi.js";

/** Max dynamic point lights summed in the display pass — the shader's loop bound and the
 *  size of the `uLightData`/`uLightColor` uniform arrays. The rig packs at most this many. */
export const MAX_HOT_LIGHTS = 32;

/** The high-shader bit that adds the lighting: carries the fragment's WORLD position
 *  (`aPosition`, unscaled world px — point-light distances need it in Phase C) and lights
 *  the albedo by the normal. `textureBit` runs BEFORE this and samples the mesh's main
 *  texture (bound to the NORMAL composite) into `outColor`, so `outColor` here IS the
 *  normal; we re-read it, sample `uAlbedo` ourselves at `vUV`, and write the lit colour. */
const lightBitGl = {
  name: "viewport-light-bit",
  vertex: {
    header: /* glsl */ `out vec2 vWorld;`,
    main: /* glsl */ `vWorld = aPosition;`,
  },
  fragment: {
    header: /* glsl */ `
      uniform sampler2D uAlbedo;                    // albedo composite (premultiplied)
      uniform vec3 uAmbient;                        // ambient floor (colour × intensity)
      uniform vec3 uSunDir;                         // normalised sun direction (world/tangent)
      uniform vec3 uSunColor;                       // sun colour × intensity
      uniform float uNormalYSign;                   // flip normal Y into the screen convention (-1)
      uniform vec4 uLightData[${MAX_HOT_LIGHTS}];   // per point light: xy world px, z height, w ±radius px (sign = casts shadow)
      uniform vec4 uLightColor[${MAX_HOT_LIGHTS}];  // per point light: rgb colour, a brightness
      uniform float uLightCount;                    // active point lights (loop breaks past it)
      uniform sampler2D uSurface;                    // G = ambient occlusion (R = height / B reserved)
      uniform vec2 uPan;                             // fillDisplay's pan: vWorld = trueWorld + uPan
      uniform sampler2D uShadow;                     // shadow coverage RT (R = shadowed 0..1), screen space
      uniform vec4 uShadowUv;                        // vWorld → shadow uv: uv = vWorld·xy + zw (zw carries any Y flip)
      in vec2 vWorld;

      // The shadow pass rasterises billboard "wedge" shadows into a screen-space coverage RT.
      // Sample it at this fragment's screen position; 0 = lit, 1 = fully shadowed. Out-of-RT
      // fragments (uv outside 0..1) read as unshadowed.
      float shadowCoverage() {
        vec2 uv = vWorld * uShadowUv.xy + uShadowUv.zw;
        if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) return 0.0;
        return texture(uShadow, uv).r;
      }
    `,
    main: /* glsl */ `
      // outColor = the NORMAL composite (textureBit). Empty (uncovered) texels read
      // (0,0,0,0) — treat those as flat-up +Z so bare cells light like flat ground.
      vec4 nTex = outColor;
      vec3 nrm = nTex.a > 0.001 ? nTex.rgb * 2.0 - 1.0 : vec3(0.0, 0.0, 1.0);
      // AMPLIFY the relief in math (instead of baking exaggerated normals): scale the
      // tangent XY away from flat-up, then renormalize. A bigger tilt widens the N·L range
      // → brighter facing-highlights + darker facing-away shadows from the SAME light. Flat
      // normals (tiles, bare cells: xy = 0) are untouched, so this only affects real relief.
      const float NORMAL_STRENGTH = 1.6;
      nrm.xy *= NORMAL_STRENGTH;
      vec3 N = normalize(vec3(nrm.x, nrm.y * uNormalYSign, nrm.z));

      // Half-Lambert wrap softens the terminator so exaggerated normal relief deepens
      // CONTRAST without clipping away-facing surfaces to black.
      const float LIGHT_WRAP = 0.4;

      // Ambient occlusion (surface.G): gates AMBIENT fully (occlusion blocks indirect light)
      // and DIRECT light only partially — a crevice still catches a key light, but darkens,
      // reading as depth/contact grime rather than flat plastic. Covered texels carry the AO;
      // uncovered ones read 0 but their albedo is 0 too, so it never shows.
      float ao = texture(uSurface, vUV).g;
      // Deepen the baked AO's contrast in-shader (the map only dips to ~0.75): scale the
      // occlusion (1 - ao) up, so crevices darken harder without re-baking the map.
      const float AO_STRENGTH = 3.0;
      ao = clamp(1.0 - (1.0 - ao) * AO_STRENGTH, 0.0, 1.0);
      const float AO_DIRECT = 0.9;
      float aoDirect = mix(1.0, ao, AO_DIRECT);
      // Accumulate DIRECT light (sun + point lights) apart from ambient, so the shadow map
      // gates only the direct contribution — ambient still fills the shadow.
      vec3 direct = vec3(0.0);

      // Directional sun: no distance falloff. Half-Lambert N·L.
      float sndl = max((dot(N, uSunDir) + LIGHT_WRAP) / (1.0 + LIGHT_WRAP), 0.0);
      direct += uSunColor * (sndl * aoDirect);

      // Up to MAX_HOT_LIGHTS point lights (world px), quadratic falloff, half-Lambert.
      // Flat loop, break on the count; only the loop var indexes the arrays.
      for (int i = 0; i < ${MAX_HOT_LIGHTS}; i++) {
        if (float(i) >= uLightCount) break;
        vec4 ld = uLightData[i];
        float radius = abs(ld.w);                       // |w|; sign reserved (was casts-shadow)
        // Lights are PURE world px; vWorld is the panned coord (trueWorld + uPan), so add
        // uPan to the light to compare them in the same space.
        vec3 toL = vec3(ld.xy + uPan - vWorld, ld.z);   // fragment → light (world px + height)
        float atten = clamp(1.0 - length(toL.xy) / max(radius, 1.0), 0.0, 1.0);
        atten *= atten;                                 // quadratic
        if (atten <= 0.0) continue;
        float ndl = max((dot(N, normalize(toL)) + LIGHT_WRAP) / (1.0 + LIGHT_WRAP), 0.0);
        direct += uLightColor[i].rgb * (uLightColor[i].a * ndl * atten * aoDirect);
      }

      // Gate the direct light by the screen-space wedge-shadow coverage (0 = lit, 1 = shadowed).
      const float SHADOW_STRENGTH = 0.7;
      float cov = shadowCoverage();
      vec3 lightSum = uAmbient * ao + direct * (1.0 - cov * SHADOW_STRENGTH);

      vec4 alb = texture(uAlbedo, vUV);                 // premultiplied → multiply is safe
      outColor = vec4(alb.rgb * lightSum, alb.a);
    `,
  },
};

let program: GlProgram | null = null;
function lightingProgram(): GlProgram {
  if (!program) {
    program = compileHighShaderGlProgram({
      name: "viewport-lighting",
      // textureBit BEFORE lightBit: outColor must hold the normal sample when lightBit runs.
      bits: [localUniformBitGl, textureBitGl, lightBitGl, roundPixelsBitGl],
    });
  }
  return program;
}

/** Unpack `0xRRGGBB` into a 3-float `[r,g,b]` in 0..1, scaled by `intensity`. */
function rgb(color: number, intensity = 1): Float32Array {
  return new Float32Array([
    (((color >> 16) & 0xff) / 255) * intensity,
    (((color >> 8) & 0xff) / 255) * intensity,
    ((color & 0xff) / 255) * intensity,
  ]);
}

/** The lit-ground shader. Bind the NORMAL composite via `normal`, the ALBEDO composite via
 *  `albedo`, and set the sun/ambient. The display mesh samples both through its `aUV`. */
export class LightingShader extends Shader {
  private _normal: Texture = Texture.EMPTY;

  /** The NORMAL composite — the mesh's MAIN texture (`textureBit` samples it into
   *  `outColor`). Identity texture-matrix: `vUV` = `aUV` (the bake stores it upright). */
  set normal(value: Texture) {
    this._normal = value;
    this.resources.uTexture = value.source;
    this.resources.uSampler = value.source.style;
  }
  /** `Mesh` requires its shader to be a `TextureShader` (expose `texture`); the mesh's main
   *  texture IS the normal composite, so this aliases {@link normal}. */
  get texture(): Texture {
    return this._normal;
  }
  set texture(value: Texture) {
    this.normal = value;
  }
  /** The ALBEDO composite — sampled by `lightBit` at `vUV`. */
  set albedo(value: Texture) {
    this.resources.uAlbedo = value.source;
    this.resources.uAlbedoSampler = value.source.style;
  }
  /** The SURFACE composite — R = height (shadow march), G = ambient occlusion, B = open.
   *  `0x00ff00` where no surface art (flat ground, no occlusion). */
  set surface(value: Texture) {
    this.resources.uSurface = value.source;
    this.resources.uSurfaceSampler = value.source.style;
  }
  /** The shadow coverage RT (R = shadowed 0..1), sampled per-fragment in screen space. */
  set shadow(value: Texture) {
    this.resources.uShadow = value.source;
    this.resources.uShadowSampler = value.source.style;
  }
  /** Map `vWorld` → shadow uv: `uv = vWorld·(sx,sy) + (ox,oy)`. The offset carries the RT's
   *  Y flip (`sy` negative, `oy` = 1) when the render target samples flipped. Set each frame
   *  (it moves with the zoom + body size). */
  setShadowUv(sx: number, sy: number, ox: number, oy: number): void {
    this.resources.lightUniforms.uniforms.uShadowUv = new Float32Array([sx, sy, ox, oy]);
    this.resources.lightUniforms.update();
  }
  /** The display pan (`fillDisplay`'s panX/panY): the per-vertex `aPosition` is
   *  `trueWorld + pan`, so world-space lights add this to compare against `vWorld`. Set
   *  each frame (it moves with the anchor). */
  setPan(panX: number, panY: number): void {
    this.resources.lightUniforms.uniforms.uPan = new Float32Array([panX, panY]);
    this.resources.lightUniforms.update();
  }
  /** Set the directional sun: `dir` is a world-space direction (normalised here), `color`
   *  is `0xRRGGBB`, `intensity` scales it. */
  setSun(dir: { x: number; y: number; z: number }, color: number, intensity: number): void {
    const len = Math.hypot(dir.x, dir.y, dir.z) || 1;
    const u = this.resources.lightUniforms.uniforms;
    u.uSunDir = new Float32Array([dir.x / len, dir.y / len, dir.z / len]);
    u.uSunColor = rgb(color, intensity);
    this.resources.lightUniforms.update();
  }
  /** Set the ambient floor (`0xRRGGBB` × intensity). */
  setAmbient(color: number, intensity: number): void {
    this.resources.lightUniforms.uniforms.uAmbient = rgb(color, intensity);
    this.resources.lightUniforms.update();
  }
  /** Replace the dynamic point-light set. `data`/`color` are {@link MAX_HOT_LIGHTS}·4-float
   *  arrays (data = xy,z,radius; color = rgb,brightness); only the first `count` are read. */
  setLights(data: Float32Array, color: Float32Array, count: number): void {
    const u = this.resources.lightUniforms.uniforms;
    u.uLightData = data;
    u.uLightColor = color;
    u.uLightCount = count;
    this.resources.lightUniforms.update();
  }
}

export function makeLightingShader(): LightingShader {
  const empty = Texture.EMPTY;
  return new LightingShader({
    glProgram: lightingProgram(),
    resources: {
      uTexture: empty.source,
      uSampler: empty.source.style,
      // Identity — vUV = aUV (composite coords), no frame/flip remap (bake is upright).
      textureUniforms: { uTextureMatrix: { type: "mat3x3<f32>", value: new Matrix() } },
      uAlbedo: empty.source,
      uAlbedoSampler: empty.source.style,
      uSurface: empty.source,
      uSurfaceSampler: empty.source.style,
      uShadow: empty.source,
      uShadowSampler: empty.source.style,
      lightUniforms: new UniformGroup({
        uAmbient: { value: new Float32Array([0.12, 0.14, 0.18]), type: "vec3<f32>" },
        uSunDir: { value: new Float32Array([-0.4, -0.5, 0.75]), type: "vec3<f32>" },
        uSunColor: { value: new Float32Array([0.6, 0.6, 0.6]), type: "vec3<f32>" },
        uNormalYSign: { value: -1, type: "f32" },
        uShadowUv: { value: new Float32Array([0, 0, 0, 0]), type: "vec4<f32>" },
        uPan: { value: new Float32Array([0, 0]), type: "vec2<f32>" },
        uLightData: { value: new Float32Array(MAX_HOT_LIGHTS * 4), type: "vec4<f32>", size: MAX_HOT_LIGHTS },
        uLightColor: { value: new Float32Array(MAX_HOT_LIGHTS * 4), type: "vec4<f32>", size: MAX_HOT_LIGHTS },
        uLightCount: { value: 0, type: "f32" },
      }),
    },
  });
}
