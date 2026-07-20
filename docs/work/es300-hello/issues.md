# Issues — es300-hello

_Gotchas to respect. The proof is small; the sharp edges are all about the raw-source plumbing._

---

## I-1 · `#version 300 es` MUST be the first line — RESOLVED: Pixi handles it — 2026-07-20

The version directive has to be the **very first line** (only comments/whitespace before it). **Pixi v8
does this for us:** `GlProgram` detects `isES300 = options.fragment.includes("#version 300 es")`, then runs
`stripVersion` (removes the directive wherever it is) → `ensurePrecision` → `addProgramDefines` (no-op for
ES3 — no WebGL1 shims) → `insertVersion` (re-prepends `#version 300 es` as line 1). So the final source is
version-first, precision-after, shim-free. No manual GL needed. The one requirement: the **fragment** must
contain the literal `#version 300 es` (it's the detection trigger); include it in the vertex too for
clarity (it's stripped + re-inserted regardless).

## I-2 · Both stages same version — no mixing ES 1.00 + ES 3.00 in one program

A program links a vertex + a fragment shader; both must be the **same GLSL version**. You can't pair Pixi's
ES 1.00 default/filter vertex with an ES 3.00 fragment — the link fails. So the vertex shader is
hand-written `#version 300 es` too (rules out the plain-filter path).

## I-3 · The ES 3.00 syntax deltas (vs the high-shader ES 1.00 we know)

- `attribute` → `in` (vertex); `varying` → `out` (vertex) / `in` (fragment).
- No `gl_FragColor` — declare your own `out vec4 fragColor;`.
- `texture2D`/`texture2DLod` → `texture`/`textureLod`; `texelFetch` is new.
- `precision …` comes **after** `#version`, not before.
- Integer varyings must be `flat` (no interpolation).
- Unsigned literals need the `u` suffix (`1u`, `0xFFu`); `uint`/`uvec*` are real types.

## I-4 · ES 1.00 and ES 3.00 coexist in one WebGL2 context

Each program is compiled/linked independently and declares its own `#version`; a WebGL2 context runs both.
So the ES 3.00 mesh does **not** disturb the rest of the Pixi scene (ES 1.00 batches, the G-buffer bakes,
the shadow shaders) — they keep rendering. The proof is additive, not a pipeline swap.

## I-5 · Pixi `Mesh` + raw `GlProgram` — confirm attribute/uniform binding

Custom meshes so far use **high-shader** programs (which carry Pixi's global-uniform bits for projection).
A raw program has none of that — it must self-contain (clip-space vertex, own uniforms). Confirm Pixi binds
the geometry's `aPosition` to the raw program's `in aPosition` (name match) and doesn't require the
high-shader uniform groups. If Pixi assumes them, that's the manual-GL signal ([F1](forks.md#f1)).

## I-6 · Prove it's ES 3.00, not ES 1.00 silently accepting it

The point is to exercise syntax ES 1.00 **cannot** compile, so a render is real proof. `uint`/bitwise is
that (ES 1.00 → `'uint' : undeclared identifier`). As a negative control, flipping the same source to
`#version 100` should fail to compile — do it once to confirm the harness isn't masking the version.
