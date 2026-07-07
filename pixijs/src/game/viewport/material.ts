//! The material system's client-side data: the per-channel bindings a prim carries,
//! the material registry (params by id, from the `<material>` DSL), and the noise-field
//! catalogue. This is the plumbing the albedo BAKE pass consumes to add hue/chroma
//! variation to a flat albedo — never lightness, so it reads as pigment, not light. See
//! `docs/lighting.md` and `content/material/materials.rd`.
//!
//! Reconstruction (in `materialBakeShader.ts`), per fragment, holding L fixed in OKLab:
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

/** Length of each `vec4[4]` packed-channel uniform array (4 channels × 4 floats). */
export const PACKED_UNIFORM_LEN = 16;

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
}

/** The material registry: `params[materialId - 1]` → its {@link MaterialDef}. Built once
 *  per content load (and on hot-swap) from the wasm parallel arrays; a `materialId` of `0`
 *  (or out of range) means "no material". */
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

  /** Whether a prim's packed channels bind any material that actually VARIES — the gate
   *  for taking the material bake path (else the prim bakes flat, as before). */
  channelsVary(channels: readonly PackedChannel[] | undefined): boolean {
    if (!channels) return false;
    return channels.some((c) => {
      const d = this.get(c.materialId);
      return !!d && (d.hueSwing !== 0 || d.chromaSwing !== 0);
    });
  }

  /** Pack a prim's channels into the two `vec4[4]` uniform arrays the bake shader reads:
   *  `chA[i] = (tintR, tintG, tintB, hueSwing)`, `chB[i] = (chromaSwing, warmCoolBias,
   *  noiseRow, sampleSpace)`. Unbound / unknown-material channels stay all-zero → the
   *  shader's delta contribution is `0`. */
  packChannels(channels: readonly PackedChannel[] | undefined): { chA: Float32Array; chB: Float32Array } {
    const chA = new Float32Array(PACKED_UNIFORM_LEN);
    const chB = new Float32Array(PACKED_UNIFORM_LEN);
    if (!channels) return { chA, chB };
    for (let i = 0; i < 4 && i < channels.length; i++) {
      const c = channels[i];
      const base = i * 4;
      chA[base] = ((c.tint >> 16) & 0xff) / 255;
      chA[base + 1] = ((c.tint >> 8) & 0xff) / 255;
      chA[base + 2] = (c.tint & 0xff) / 255;
      const d = this.get(c.materialId);
      chA[base + 3] = d?.hueSwing ?? 0;
      chB[base] = d?.chromaSwing ?? 0;
      chB[base + 1] = d?.warmCoolBias ?? 0;
      chB[base + 2] = d?.noiseFieldIndex ?? NO_NOISE_FIELD;
      chB[base + 3] = d?.sampleSpace ?? 0;
    }
    return { chA, chB };
  }

  /** Build from the wasm registry's parallel arrays: noise-field NAMES + sample spaces
   *  (`materialSampleSpaces`) + flat stride-3 swings (`materialSwings`:
   *  `[hueDeg, chroma, warmCool, …]`). Degrees → radians here so the shader stays in
   *  radians. */
  static fromWasm(noiseFields: string[], sampleSpaces: string[], swings: Float64Array): MaterialRegistry {
    const defs: MaterialDef[] = noiseFields.map((field, i) => ({
      noiseFieldIndex: noiseFieldIndex(field),
      hueSwing: ((swings[i * 3] ?? 0) * Math.PI) / 180,
      chromaSwing: swings[i * 3 + 1] ?? 0,
      warmCoolBias: swings[i * 3 + 2] ?? 0,
      sampleSpace: sampleSpaces[i] === "world" ? 1 : 0,
    }));
    return new MaterialRegistry(defs);
  }
}
