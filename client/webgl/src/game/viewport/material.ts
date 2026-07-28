//! The material system's client-side data: the per-channel bindings a prim carries,
//! the material registry (params by id, from the `<material>` DSL), and the noise-field
//! catalogue. This is the plumbing the albedo BAKE pass consumes to add hue/chroma
//! variation to a flat albedo — never lightness, so it reads as pigment, not light. See
//! `docs/lighting.md` and `content/material/materials.rd`.
//!
//! Reconstruction (in `mrtBakeShader.ts`), per fragment, holding L fixed in OKLab:
//!   out.rgb = albedo.rgb + Σ packedᵢ · (jitter(tintᵢ, noise, paramsᵢ) − tintᵢ)
//!   out.a   = albedo.a
//! The DELTA form is identity-safe: an unbound channel (`tint = 0`) or a zero-swing
//! material contributes exactly `0`, so a stem with a packed map but no material bindings
//! bakes its albedo unchanged.

/** The noise-field catalogue, in ATLAS order — the row index each field occupies in the
 *  bundled noise atlas (`noiseAtlas.ts`). A material's `noiseField` name resolves to its
 *  index here; an unknown/absent name resolves to {@link NO_NOISE_FIELD} (→ the shader
 *  samples a neutral 0.5 = no perturbation). Extend by APPENDING (atlas rows are
 *  positional) as `bin/lib/noise_fields.py` grows the set. */
export const NOISE_FIELDS = ["mottle", "strand"] as const;
export type NoiseFieldName = (typeof NOISE_FIELDS)[number];

/** Sentinel for "no noise field" — the shader treats it as a constant 0.5 (identity). */
export const NO_NOISE_FIELD = -1;

/** Length of each `vec4[3]` layer-channel uniform array (3 channels × 4 floats). The 4th
 *  (alpha) material channel was dropped — the `layers` map is RGB. */
export const PACKED_UNIFORM_LEN = 12;

/** Resolve a material's `noiseField` name to its atlas row index, or {@link NO_NOISE_FIELD}. */
export function noiseFieldIndex(name: string): number {
  const i = (NOISE_FIELDS as readonly string[]).indexOf(name);
  return i >= 0 ? i : NO_NOISE_FIELD;
}

/** One packed-map channel's material binding on a prim (from `Content.tilePackedChannels`).
 *  `materialId` is 1-based into the registry (`0` = no material → flat tint, no jitter);
 *  `tint` is the channel's base colour `0xRRGGBB` (what `split_layers` subtracted, so the
 *  delta form perturbs AROUND it). */
export interface PackedChannel {
  materialId: number;
  tint: number;
}

/** A registered material's render params (the `<material>` def, mirrored from wasm).
 *  `noiseFieldIndex` is pre-resolved to an atlas row; swings drive OKLab hue/chroma jitter
 *  (never L); `sampleSpace` is `0` = UV (rides the sprite) or `1` = world (pinned to the
 *  ground). All-zero swings = identity. Indexed by `materialId - 1`. */
export interface MaterialDef {
  noiseFieldIndex: number;
  /** Hue rotation amplitude at full noise, in RADIANS (converted from the DSL's degrees). */
  hueSwing: number;
  chromaSwing: number;
  warmCoolBias: number;
  sampleSpace: 0 | 1;
  /** NORMAL-DETAIL (material-system P1): the detail field's atlas row ({@link NO_NOISE_FIELD} =
   *  none), its amplitude (0 = identity), and its UV-tiling scale multiplier. */
  detailFieldIndex: number;
  detailAmp: number;
  detailScale: number;
}

/** The material registry: `params[materialId - 1]` → its {@link MaterialDef}. Built once
 *  per content load (and on hot-swap) from the wasm parallel arrays; a `materialId` of `0`
 *  (or out of range) means "no material". */
/** Per-layer-channel DEFAULT tint when the content authored none — three DISTINCT grays so a
 *  split sprite with no authored tints reconstructs its materials as separable gray masses
 *  (readable form + shading, no hue) instead of baking the colour-stripped residual. The
 *  reconstruction (`residual + Σ layersᵢ·tintᵢ`) scales each by that channel's per-pixel weight,
 *  so a default gray only shows where its material actually is. An authored `packed.N.tint`
 *  overrides it (to restyle/recolour that material). `split_layers` throws away the true base
 *  colours it computed, so these grays stand in until the pipeline emits them (see notes). */
const DEFAULT_LAYER_GRAY = [0x666666, 0x8c8c8c, 0xb3b3b3];

export class MaterialRegistry {
  private readonly defs: MaterialDef[];

  constructor(defs: MaterialDef[]) {
    this.defs = defs;
  }

  /** The def for a 1-based `materialId`, or `undefined` for `0` / out of range. */
  get(materialId: number): MaterialDef | undefined {
    return materialId > 0 ? this.defs[materialId - 1] : undefined;
  }

  /** Whether any registered material actually varies (non-zero swing) — lets the bake
   *  skip the material path entirely when the corpus authored no real materials. */
  get hasAnyVariation(): boolean {
    return this.defs.some((d) => d.hueSwing !== 0 || d.chromaSwing !== 0);
  }

  /** Pack a prim's channels into the two `vec4[4]` uniform arrays the bake shader reads:
   *  `chA[i] = (tintR, tintG, tintB, hueSwing)`, `chB[i] = (chromaSwing, warmCoolBias,
   *  noiseRow, sampleSpace)`. ALL 3 layer channels are always emitted: a channel with an
   *  authored tint uses it (a restyle/recolour); a channel with none falls back to that
   *  channel's {@link DEFAULT_LAYER_GRAY}, so an unauthored split sprite reconstructs its
   *  materials as distinct grays rather than the bare residual. The per-pixel layer weight
   *  scales each contribution in the shader, so a gray only shows where its material is. */
  packChannels(channels: readonly PackedChannel[] | undefined): { chA: Float32Array; chB: Float32Array; chC: Float32Array } {
    const chA = new Float32Array(PACKED_UNIFORM_LEN);
    const chB = new Float32Array(PACKED_UNIFORM_LEN);
    const chC = new Float32Array(PACKED_UNIFORM_LEN);
    // 3 layer channels (RGB); any 4th binding on a prim is ignored (the `layers` map is RGB).
    for (let i = 0; i < 3; i++) {
      const c = channels?.[i];
      const base = i * 4;
      // Authored tint (nonzero) → restyle; else the per-channel default gray.
      const tint = c && c.tint > 0 ? c.tint : DEFAULT_LAYER_GRAY[i];
      chA[base] = ((tint >> 16) & 0xff) / 255;
      chA[base + 1] = ((tint >> 8) & 0xff) / 255;
      chA[base + 2] = (tint & 0xff) / 255;
      const d = c ? this.get(c.materialId) : undefined;
      chA[base + 3] = d?.hueSwing ?? 0;
      chB[base] = d?.chromaSwing ?? 0;
      chB[base + 1] = d?.warmCoolBias ?? 0;
      chB[base + 2] = d?.noiseFieldIndex ?? NO_NOISE_FIELD;
      chB[base + 3] = d?.sampleSpace ?? 0;
      // chC (material-system P1): (detail field row, detail amp, detail scale, placement mode).
      // Mode 0 = UV until F1's by-eye pick lands (P4).
      chC[base] = d?.detailFieldIndex ?? NO_NOISE_FIELD;
      chC[base + 1] = d?.detailAmp ?? 0;
      chC[base + 2] = d?.detailScale ?? 1;
      chC[base + 3] = 0;
    }
    return { chA, chB, chC };
  }

  /** Build from the wasm registry's parallel arrays: noise-field NAMES + sample spaces
   *  (`materialSampleSpaces`) + flat stride-3 swings (`materialSwings`:
   *  `[hueDeg, chroma, warmCool, …]`). Degrees → radians here so the shader stays in
   *  radians. */
  static fromWasm(noiseFields: string[], sampleSpaces: string[], swings: Float64Array,
                  detailFields?: string[], detail?: Float64Array): MaterialRegistry {
    const defs: MaterialDef[] = noiseFields.map((field, i) => ({
      noiseFieldIndex: noiseFieldIndex(field),
      hueSwing: ((swings[i * 3] ?? 0) * Math.PI) / 180,
      chromaSwing: swings[i * 3 + 1] ?? 0,
      warmCoolBias: swings[i * 3 + 2] ?? 0,
      sampleSpace: sampleSpaces[i] === "world" ? 1 : 0,
      // material-system P1: the stride-2 `materialDetail` parallel array ([amp, scale]).
      detailFieldIndex: noiseFieldIndex(detailFields?.[i] ?? ""),
      detailAmp: detail?.[i * 2] ?? 0,
      detailScale: detail?.[i * 2 + 1] ?? 1,
    }));
    return new MaterialRegistry(defs);
  }
}
