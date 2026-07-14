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
      uniform sampler2D uAlbedo;                    // albedo composite (OPAQUE colour; coverage applied at output)
      uniform vec3 uAmbient;                        // ambient floor (colour × intensity)
      uniform vec3 uSunDir;                         // normalised sun direction (world/tangent)
      uniform vec3 uSunColor;                       // sun colour × intensity
      uniform float uNormalYSign;                   // flip normal Y into the screen convention (-1)
      uniform vec4 uLightData[${MAX_HOT_LIGHTS}];   // per point light: xy world px, z height, w ±radius px (sign = casts shadow)
      uniform vec4 uLightColor[${MAX_HOT_LIGHTS}];  // per point light: rgb colour, a brightness
      uniform float uLightCount;                    // active point lights (loop breaks past it)
      uniform sampler2D uSurface;                    // OPAQUE: R = presence, G = ambient occlusion, B = alpha (coverage)
      uniform sampler2D uDepth;                      // zdepth-world-cold (OPAQUE): B = thing tile-Y depth, R = cold shadow (reserved)
      uniform sampler2D uAlbedoWarm;                 // WARM tier: mover albedo (composited over cold by warm coverage)
      uniform sampler2D uNormalWarm;                 // WARM tier: mover normal
      uniform sampler2D uSurfaceWarm;                // WARM tier: mover surface (its B = the composite coverage)
      uniform sampler2D uDepthWarm;                  // WARM tier: mover zdepth-world
      uniform vec2 uPan;                             // fillDisplay's pan: vWorld = trueWorld + uPan
      uniform sampler2D uShadow;                     // zdepth_screen (screen space, OPAQUE): G = caster tile-Y depth (5+row%251); 0..4 = no shadow
      uniform vec4 uShadowUv;                        // vWorld → shadow uv: uv = vWorld·xy + zw (zw carries any Y flip)
      in vec2 vWorld;

      // The shadow pass (zdepth_screen) rasterises billboard "wedge" shadows into a screen-space
      // RT — G = the caster's tile-Y depth (5 + row%251), 0..4 = no shadow. Sample it at this
      // fragment's screen position and return the direct-light coverage (0 = lit, 1 = shadowed).
      //
      // Silhouettes are HARD (AA off): presence (surface.R) is a clean 0/1, so the receiver
      // classifies binary. A present thing sitting at/in FRONT of the caster blocks the shadow
      // with its own body (self-omit); ground, or anything behind the caster, is shadowed. Depths
      // ride the 251-ring (3 steps per tile), so the ordering is the WRAPPED difference. Out-of-RT
      // fragments read as unshadowed.
      float shadowCoverage(float receiverDepth, float presence) {
        vec2 uv = vWorld * uShadowUv.xy + uShadowUv.zw;
        if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) return 0.0;
        vec4 sh = texture(uShadow, uv);
        if (sh.g < 0.02) return 0.0;                    // reserved 0..4 band (G < 5/255) = no shadow
        if (presence < 0.5) return 1.0;                 // ground receiver → fully shadowed
        // Both the receiver (zdepth_world.B) and the caster (zdepth_screen.G) encode 5 + row%251,
        // where row is a SUB-TILE step (3 per tile). Recover the ring index (0..250) and take the
        // NEAREST wrap of the difference over the 251-ring, so a low value renders over a high value
        // (the wrap-around). Unambiguous for shadows up to ~41 tiles long; beyond that the wrap
        // misclassifies (fine for now).
        float rRecv = receiverDepth * 255.0 - 5.0;      // this thing's sub-tile row
        float rCast = sh.g * 255.0 - 5.0;               // caster's sub-tile row
        float d = rRecv - rCast;
        d -= 251.0 * floor(d / 251.0 + 0.5);            // wrap into (-125.5, 125.5]
        // Receiver AT / IN FRONT of the caster (d >= 0) blocks the shadow with its own body — the
        // -0.5 threshold self-omits the caster's own sub-tile band while a neighbour ONE sub-tile
        // behind (d = -1) is still shadowed. Behind the caster → the whole fragment takes the shadow.
        return d >= -0.5 ? 0.0 : 1.0;
      }
    `,
    main: /* glsl */ `
      // WARM-over-COLD composite: the warm tier (movers) is slot-aligned with cold (same
      // window), so sample it at the SAME vUV and blend by warm coverage (warm surface.B). Where
      // no mover sits, warm coverage is 0 → pure cold. Done up front so the ENTIRE G-buffer
      // (normal/albedo/surface/depth) is the merged value and the lighting below is tier-agnostic
      // — pawns light + shadow exactly like the world.
      float wcov = texture(uSurfaceWarm, vUV).b;

      // outColor = the COLD NORMAL composite (textureBit), now OPAQUE raw normals. Merge the warm
      // normal over it, then decode straight; near-black texels (an empty/cleared cell) → flat-up
      // +Z so they light like flat ground.
      vec4 nTex = mix(outColor, texture(uNormalWarm, vUV), wcov);
      vec3 nrm = length(nTex.rgb) < 0.02 ? vec3(0.0, 0.0, 1.0) : nTex.rgb * 2.0 - 1.0;
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

      // OPAQUE composites — read straight, NO un-premultiply. albedo = colour; surface carries
      // R = presence, G = ao, B = alpha (coverage). The visual alpha is applied to the OUTPUT
      // only (never stored in a map), so alpha composites the final pixel over the background.
      vec4 alb = mix(texture(uAlbedo, vUV), texture(uAlbedoWarm, vUV), wcov);
      vec4 surf = mix(texture(uSurface, vUV), texture(uSurfaceWarm, vUV), wcov);
      float presence = surf.r;                          // 1 = a thing occupies this fragment (hard)
      float alpha = surf.b;                             // visual coverage → output alpha
      float ao = surf.g;
      // Ambient occlusion gates AMBIENT fully (occlusion blocks indirect light) and DIRECT light
      // only partially — a crevice still catches a key light but darkens, reading as contact grime.
      // zdepth-world: B = the fragment's thing tile-Y depth (0 = ground). Warm over cold.
      vec4 dc = mix(texture(uDepth, vUV), texture(uDepthWarm, vUV), wcov);
      float receiverDepth = dc.b;
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
      // Pass the fragment's opacity so a shadow behind a transparent thing shows through it.
      const float SHADOW_STRENGTH = 0.7;
      float cov = shadowCoverage(receiverDepth, presence);
      vec3 lightSum = uAmbient * ao + direct * (1.0 - cov * SHADOW_STRENGTH);

      // Coverage (visual alpha) is applied at OUTPUT only — premultiplied so it composites over
      // the canvas background. Opaque maps in, alpha out: empty cells (α 0) show the background,
      // ground/things (alpha 1) draw opaque. This is the (albedo + layers*tint)*alpha of the spec.
      outColor = vec4(alb.rgb * lightSum * alpha, alpha);
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
  /** The DEPTH composite — B = the fragment's own thing tile-depth (0 = ground), silhouette-
   *  accurate. The shadow compare omits a shadow where this ≥ the caster depth. */
  set depth(value: Texture) {
    this.resources.uDepth = value.source;
    this.resources.uDepthSampler = value.source.style;
  }
  /** WARM-tier composites (movers), blended over their cold counterparts by warm coverage
   *  (warm surface.B) at the top of the fragment. Left EMPTY (coverage 0 → pure cold) until the
   *  warm cache is ready. */
  set albedoWarm(value: Texture) {
    this.resources.uAlbedoWarm = value.source;
    this.resources.uAlbedoWarmSampler = value.source.style;
  }
  set normalWarm(value: Texture) {
    this.resources.uNormalWarm = value.source;
    this.resources.uNormalWarmSampler = value.source.style;
  }
  set surfaceWarm(value: Texture) {
    this.resources.uSurfaceWarm = value.source;
    this.resources.uSurfaceWarmSampler = value.source.style;
  }
  set depthWarm(value: Texture) {
    this.resources.uDepthWarm = value.source;
    this.resources.uDepthWarmSampler = value.source.style;
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
      uDepth: empty.source,
      uDepthSampler: empty.source.style,
      uAlbedoWarm: empty.source,
      uAlbedoWarmSampler: empty.source.style,
      uNormalWarm: empty.source,
      uNormalWarmSampler: empty.source.style,
      uSurfaceWarm: empty.source,
      uSurfaceWarmSampler: empty.source.style,
      uDepthWarm: empty.source,
      uDepthWarmSampler: empty.source.style,
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
