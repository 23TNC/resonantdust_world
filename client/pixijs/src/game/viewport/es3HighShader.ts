//! es300-migration — the ONE thing we port from Pixi to get every shader onto GLSL ES 3.00.
//!
//! Pixi's high-shader GL templates are already written in a version-agnostic style (`in`/`out`/`texture`/
//! `finalColor`) that it *shims down* to ES 1.00 with `#define in varying` etc. Its `GlProgram` skips those
//! shims and compiles native ES 3.00 the moment the FRAGMENT contains `#version 300 es`. So the whole
//! migration is: assemble the program exactly as before (`compileHighShaderGlProgram` — same bits, same
//! vertex/uniform/roundPixels plumbing), but inject `#version 300 es` into the fragment. No hand-rolled
//! vertex, no reverse-engineered uniform block — Pixi's own plumbing rides along unchanged.
//!
//! Migrate a shader by swapping `compileHighShaderGlProgram(` → `compileHighShaderGlProgramES300(`.

import { compileHighShaderGlProgram, type GlProgram } from "pixi.js";

type CompileArgs = Parameters<typeof compileHighShaderGlProgram>[0];

/** A bit whose only job is to plant `#version 300 es` in the assembled fragment. `GlProgram` detects it,
 *  strips it, drops the WebGL1 shims, and re-inserts the directive as line 1 → native ES 3.00. */
const ES300_BIT: CompileArgs["bits"][number] = {
  name: "es300-version",
  fragment: { header: "#version 300 es\n" },
};

/** Drop-in for {@link compileHighShaderGlProgram} that emits GLSL ES 3.00 instead of ES 1.00 — the client's
 *  one shader standard. Same bits, same behaviour; the templates are version-agnostic so nothing else
 *  changes (bit ops can now use real `uint`, but that's an optional per-shader cleanup, not required here). */
export function compileHighShaderGlProgramES300(opts: CompileArgs): GlProgram {
  return compileHighShaderGlProgram({ name: opts.name, bits: [ES300_BIT, ...opts.bits] });
}
