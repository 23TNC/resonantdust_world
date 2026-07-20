# Work — es300-hello (the first hand-crafted GLSL ES 3.00 shader)

_Opened 2026-07-20. De-risks the raw-shader capability that [`caster-lut` C5](../caster-lut/todo.md) (and
future integer-texture / `texelFetch` / MRT work) depends on. Component:
[`client/pixijs`](../../components/client/pixijs/). Nothing in the client renders through an ES 3.00 shader
today — every shader is Pixi's **high-shader (GLSL ES 1.00)**, which is why `uint` fails to compile (the
recurring finding). This proves we can hand-write a raw `#version 300 es` program, render it into the
viewport, and that ES-3.00-only syntax actually runs._

## Why — the GPU cast needs a dialect we've never compiled

Pixi's `compileHighShaderGlProgram` emits **GLSL ES 1.00** with WebGL1-compat shims (`#define in varying`,
`#define texture texture2D`, `gl_FragColor`) even on our WebGL2 context — so no `uint`, no bitwise ops, no
`texelFetch`, no integer textures, no MRT. `caster-lut` C5 (an instanced cast reading the caster/LUT
textures via vertex-texture-fetch) and any real integer-bitfield work need **ES 3.00**, which means
**hand-writing raw shader source** (`#version 300 es` as line 1) and handing it to Pixi as a `GlProgram`
rather than templating it. This experiment isolates that mechanism: get *one* raw ES 3.00 program compiling
and drawing, before anything depends on it.

## What — a full-viewport quad drawn by a raw ES 3.00 program

A `Mesh` + `Shader` wrapping a raw `GlProgram` (vertex **and** fragment both `#version 300 es`), added to
the viewport, toggled by a `/es300` command. The fragment draws an unmistakable pattern (a checkerboard)
using **ES-3.00-only syntax**, so a successful render is proof — the same source under ES 1.00 would not
compile.

### The proof — use what ES 1.00 can't

```glsl
#version 300 es        // ES 1.00 has no #version 100-plus dialect with these types
precision highp float;
out vec4 fragColor;    // not gl_FragColor
void main() {
  uvec2 p = uvec2(gl_FragCoord.xy) / 16u;   // uint / uvec2 — undeclared in ES 1.00
  uint checker = (p.x ^ p.y) & 1u;           // bitwise ^ & and the `1u` literal — ES 3.00 only
  fragColor = checker == 1u ? vec4(0.15, 0.75, 1.0, 1.0) : vec4(0.04, 0.08, 0.16, 1.0);
}
```

`uint`, `uvec2`, `^`, `&`, `1u`, and the user-declared `out` are exactly the constructs that throw
`'uint' : undeclared identifier` under the high-shader (see the overlay/bitfield history). If the
checkerboard appears, ES 3.00 is genuinely live. The vertex shader is a clip-space passthrough (a
full-screen quad at `[-1,1]`, no projection matrix needed):

```glsl
#version 300 es
in vec2 aPosition;
void main() { gl_Position = vec4(aPosition, 0.0, 1.0); }
```

## How it fits the pipeline

The client's WebGL2 context (`preferWebGLVersion: 2` in `main.ts`) can run ES 1.00 and ES 3.00 programs
**side by side** — each program declares its own `#version`, and they're independent ([I-4](issues.md#i-4)).
So this ES 3.00 mesh coexists with the rest of the (ES 1.00) Pixi scene; nothing else changes. The
raw-source path already exists for filters (`GlProgram.from` in `shadowCastShaders.ts`); this uses it for a
`Mesh`'s shader with **ES 3.00** source ([F1](forks.md#f1)).

## Alignment

Unlocks [`caster-lut` C5](../caster-lut/todo.md) — the instanced GPU cast that samples the light/LUT/caster
textures via vertex-texture-fetch — and every later use of `texelFetch`, integer textures, real `uint`
bitfields, and MRT. This is the foundation those build on.

## State

Phased in [`todo.md`](todo.md); the pipeline calls in [`forks.md`](forks.md); the version-directive /
coexistence gotchas in [`issues.md`](issues.md). Scope is deliberately tiny — compile + draw one ES 3.00
program — so the raw-shader mechanism is proven in isolation, not tangled into the cast.
