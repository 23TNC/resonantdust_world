# Work — es300-migration (unify every client shader to GLSL ES 3.00)

_Opened 2026-07-20. Follows [`es300-hello`](../es300-hello/README.md), which proved a raw `#version 300 es`
program compiles + renders here. Component: [`client/pixijs`](../../components/client/webgl/). Migrates
**all** existing shaders off Pixi's high-shader (ES 1.00) onto raw ES 3.00, so the client has **one shader
standard** — no ES1-vs-ES3 split, and the ES 3.00 feature set (`uint`/bitwise, `texelFetch`, integer
textures, MRT) is uniformly on the table for every shader._

## Why — one dialect, and stop faking what ES 3.00 gives natively

Every shader today is `compileHighShaderGlProgram` → **GLSL ES 1.00 with WebGL1 shims** (`#define in
varying`, `gl_FragColor`, no `uint`). That split forces two mental models and blocks the whole feature set
behind a per-shader rewrite. Unifying to ES 3.00 removes the split and, where it helps, lets us drop the
ES-1.00 workarounds — most visibly the shadow bitfield shaders' **float-mod bit math** becomes real
`uint`/bitwise. `es300-hello` proved the mechanism (Pixi detects `#version 300 es` in the fragment and
handles the version/precision/shims itself; ES1 + ES3 coexist in the one WebGL2 context).

## Scope — dialect unification, behaviour-preserving

**In scope:** every shader emits raw `#version 300 es`; the ES-1.00-only cleanups that are *behaviour-
preserving* (`texture2D`→`texture`, `gl_FragColor`→`out`, and float-mod → `uint` bitwise **reading the same
unorm8 textures**). Output must stay **pixel-identical** ([I-5](issues.md#i-5)).

**Out of scope (follow-ons this migration *enables*, not does):** switching the shadow RTs to **integer
textures** (`RGBA8UI`/`RGBA32UI` → 32/128-bit fields) belongs to [`caster-lut` C5](../caster-lut/todo.md)
(it forces the shader cast + drops fixed-function blend); folding the four **bake passes into one via MRT**
is its own experiment. This folder is the groundwork that makes both uniformly reachable. Texture formats
stay unorm RGBA8, A held at 1 ([I-7](issues.md#i-7)).

## The linchpin — a shared ES 3.00 program builder

The high-shader bits gave every shader Pixi's **camera plumbing for free**: `uProjectionMatrix` /
`uWorldTransformMatrix` / roundPixels / texture conventions. A raw ES 3.00 shader must supply that itself.
Rather than hand-roll it per shader, build **one** helper — our minimal ES 3.00 equivalent of
`compileHighShaderGlProgram`: a standard vertex that consumes Pixi's `globalUniforms` block (+ the mesh's
local transform) and does the standard transform/roundPixels, plus a fragment header convention, so each
shader supplies only its fragment body + extra uniforms. This is the one hard piece ([I-1](issues.md#i-1));
once it works, every migration is mechanical. `es300-hello` sidestepped it with a clip-space passthrough —
fine for a full-screen quad, but the bakes and world-aligned displays need the real transform.

## The shaders (8 programs + 1 filter)

| Shader | Space | ES 3.00 cleanup |
|---|---|---|
| `albedoBlitShader` | screen display | dialect only |
| `overlayShader` | screen display | dialect; BITS mode float-mod → `uint` |
| `ShadowMergeShader` | world buffer | dialect; bit clear/OR → `uint` |
| `ShadowTDisplayShader` | screen display | dialect; decode → `uint`; light-data sample → `texelFetch` |
| `makeShadowDecodeFilter` | filter | dialect (ES3 filter needs an ES3 vertex too — [I-6](issues.md#i-6)) |
| `materialBakeShader` | prim→slot (transform) | dialect |
| `surfaceBakeShader` | prim→slot (transform) | dialect |
| `normalBakeShader` | prim→slot (transform) | dialect |
| `depthBakeShader` | prim→slot (transform) | dialect |

## Alignment

Unifying the dialect is the enabler for the ranked ES 3.00 wins: the shadow bitfield's integer-texture
ceiling lift (via `caster-lut` C5) and the bake MRT collapse — both become uniform, in-standard changes
once nothing is on ES 1.00. It also retires the `webgl2-es300` two-worlds caveat.

## State

Phased in [`todo.md`](todo.md) (builder first, bakes last); the builder/scope/rollout calls in
[`forks.md`](forks.md); the transform-plumbing + behaviour-preservation gotchas in [`issues.md`](issues.md).
Incremental and safe — ES 1.00 and ES 3.00 coexist, so migrate one shader at a time with the tree green
throughout ([I-4](issues.md#i-4)).
