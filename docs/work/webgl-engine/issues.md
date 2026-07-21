# Issues — webgl-engine

_The sharp edges of owning the renderer. Most of the client is already ours; the risk is concentrated in the
render backend + keeping parity during the migration._

---

## I-1 · State management is the hard part Pixi did for us

Tracking blend / scissor / viewport / bound program / VAO / target / textures — and only issuing GL calls
when they change — is the non-trivial thing Pixi provides, and getting it subtly wrong is the classic
custom-renderer bug (a stale bind → wrong draw). BUT: owning the **whole** loop is what makes it tractable —
there's no foreign framework whose cache we corrupt (the exact failure that sank raw-GL-in-Pixi,
[caster-lut D-2](../caster-lut/deviations.md)). Start explicit (set state per draw), then add a minimal
tracked cache ([F3](forks.md#f3)). Test with a state-torture scene (many targets/programs/blends per frame).

## I-2 · The texture/atlas seam

The atlas packer + LOD are ours, but they currently wrap Pixi `Texture`/`TextureSource` and read
`Texture.frame` for the sub-image UV. Port to the engine's `Texture`: upload (from the fetched bytes /
ImageBitmap) + hold the atlas-page GL texture + expose the frame UV rect (we already compute it). Keep the
resolver's tier logic (master→preview→geo) untouched — only the texture *object* changes.

## I-3 · Keep the pixijs client working throughout

`client/pixijs` must stay building + running until W6 parity. Develop `client/webgl` in parallel; don't
share mutable state or break the pixijs build. Only W6 flips primary + archives pixijs. (The docs-authority
hooks still gate both.)

## I-4 · Parity is the cutover bar — verify feature-for-feature

The new client must match pixijs on: login → world (bakes, display, zoom/LOD, pan) → panels
(chat/debug/settings/RT) → `/shadowcast` → `/overlayRT` per channel → `/showRT` → `/es300` → all debug
commands. Build a checklist and tick it in the browser (side-by-side with pixijs) before cutover — a missed
path is a regression shipped.

## I-5 · Shader GLSL copies; only the harness is rewritten

The shader *logic* (material reconstruction, the bakes, the shadow merge/display, OKLab) is already ES 3.00
and correct — **copy the GLSL bodies verbatim** into the engine's `Program`. Rewrite ONLY the harness that
`compileHighShaderGlProgramES300`/high-shader gave us: the vertex transform (uProjectionMatrix·world·local
→ our camera uniforms), the sampler/uniform binding, roundPixels. Diffing the GLSL against pixijs guards
against visual regressions.

## I-6 · The WASM client-core seam

The pixijs client drives the game via the Rust `client/core` compiled to WASM (login, commands, events,
zone subscribe). `client/webgl` reuses the **same** WASM module + its API — this is a copy of the loading +
bridge glue, renderer-agnostic. Confirm the vite wasm plumbing + the `/content` + `/textures` fetch seams
carry over unchanged.

## I-7 · Scope honesty — this is the rest of the engine

Owning the renderer means owning: context loss/restore, resize/DPR, the frame budget, texture memory, and
the draw pipeline — things Pixi handled quietly. Phase it (W1–W7) so each is a verifiable milestone, and
don't chase Pixi's full generality — build only what THIS client uses (the copy-vs-rewrite table in the
README is the scope boundary). "Bounded" ≠ "small".
