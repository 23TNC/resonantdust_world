# De-lighting (albedo extraction)

**Status:** Marigold now generates the whole derived map set — **albedo**
(`delight.py`, IID lighting), **normals** (`normals.py`), and **depth**
(`depth.py`, 16-bit height field) — all wired through `art maps` → `art remaster`
(including grid categories like `linked`). Laigter is retired to a fallback
(`art normal --engine laigter`). Verified on real content (walls + conifer) on the
2080 Ti. The **renderer does not yet consume normals or depth**, so the shipped
maps have no lighting pass to bring form/shadows back — that pass is the real
blocker and the next piece of work. This doc records the problem, why we chose
Marigold, the licensing, the map architecture, and how the tooling is put together.

## Why de-lighting exists

Sprites (increasingly AI-generated) arrive with lighting **painted in** —
RimWorld-style baked shadows, bright edges, directional highlights. That reads
well as a flat sprite, but it fights a dynamic renderer: if the engine relights a
sprite from its normal map, the baked light double-counts with the engine's light.
De-lighting recovers the **albedo** (base colour with the lighting removed) so the
renderer can light it itself.

The goal is a pipeline with **as little hand-authoring as possible**: take a
generated sprite, run it through, and get readable, low-error albedo/normal maps
that no one notices were machine-processed. That rules out the PBR-authoring flow
(hand-painting a flat albedo layer per sprite) and pushes us to algorithmic
de-lighting.

The old approach (`art delight-divide`, still in `bin/art` for comparison) was a
two-pass **divide**: flat-field out a blurred luminance envelope, then divide out
the `N·L` shading the normal predicts. It has three structural faults:

1. Division can't remove **additive** highlights (painted rim lights) — they
   survive as smears.
2. Laigter's normal is wrong at painted albedo edges, so the normal-guided divide
   **halos** exactly where the form is crispest.
3. A Gaussian envelope isn't edge-aware, producing a bright-centre **vignette**.

On coloured organic art (conifer) the divide barely de-lit at all — it left almost
all the baked shading in the "albedo." So we replaced it with a learned model.

## The choice: Marigold-IID (lighting)

[Marigold Intrinsic Image Decomposition][mg], the **lighting** checkpoint
`prs-eth/marigold-iid-lighting-v1-1`, a diffusion model fine-tuned from Stable
Diffusion 2. It decomposes an image as

    I = A · S + R

- **A — albedo**: base colour, de-lit. Shipped as `N.albedo.png`.
- **S — diffuse shading**: the removed lighting (ambient occlusion **plus**
  directional light). Emitted as `N.albedo_shading.png` when wanted.
- **R — non-diffuse residual**: additive highlights (rim lights, speculars) **and
  self-lit glow** — the component a divide can *never* recover. This is where a
  glowing eye's colour lands (not the albedo). Emitted as `N.albedo_residual.png`;
  `art emissive` gates + cleans it into the shipped `N.emissive.png` (see below).

Because it's a learned decomposition it needs **no normal map** (unlike the
divide, which measured `N·L`). So normal and albedo generation are now
independent.

### Why not the alternatives

| Model | License | Verdict |
|---|---|---|
| **Marigold-IID-Lighting** | CreativeML Open RAIL++-M | ✅ commercial-OK, free, we own outputs |
| compphoto/Intrinsic | academic-only + **patent pending** | ❌ any commercial/product use needs an SFU licence; the patent can reach the *method*, not just the code |
| CHORD (Ubisoft La Forge) | Ubisoft ML — Research-Only, **Copyleft** | ❌ non-commercial; copyleft can attach to derivatives |

Marigold's licence (the SD2 licence) permits commercial use royalty-free, hands us
the outputs, and its only restrictions are a harmful-use blocklist (disinformation,
discrimination, etc.) that a texture pipeline never touches. None of our
monetisation plans (free game, tip jar, Patreon, paid early access, patron test
servers) affect it — OpenRAIL has no revenue or gating clauses. The only
obligations bite on redistributing **the model** (pass the licence + restrictions
downstream); shipping baked **albedo PNGs** doesn't trigger them. Keep the licence
file with the tooling and don't repackage the weights.

## The map architecture we settled on

For a normal-lit (deferred-ish) renderer, the core shipped map set is **albedo +
normal**. Depth is *not* shipped (see below); **emissive** (below) is also generated
now and is a shipping candidate (which maps go to R2 vs. stay local for dev is still
TBD). Master generation additionally produces the intermediates `albedo_residual`
(emissive's source) and, when packing tints, `packed` + `packed_residual` — kept
local, not necessarily shipped:

- **albedo** (Marigold IID) — flat base colour, no lighting.
- **emissive** — additive self-lit glow, gated from Marigold's non-diffuse residual
  (`albedo_residual`) by `art emissive`; black on matte assets. Added *after* the
  lighting pass, unlit. This is where glowing eyes / lava / runes live — the colour
  Marigold pulls out of the albedo. Dormant until the renderer's lighting pass exists.
- **normal** (Laigter, default) — tangent-space surface normals; the *shading* input
  (N·L). We use **Laigter** as canonical because we're 2.5D, not 3D: a sprite is a
  flat plane with relief, and Laigter's luminance-heightfield normal *is* the
  gradient of a single height field — so depth integrates cleanly from it (below) and
  it works on flat art. Marigold's view-space normals are cleaner macro shape but
  aren't a valid height field (steep tilts → non-integrable) and are starved on flat
  art; reach for them via `art normal --marigold`. **Convention (verified, not
  assumed):** both engines emit the SAME OpenGL / +Y-up encoding — red-high = +X
  (right), green-high = +Y (up), blue-high = +Z (toward viewer). Measured by driving
  Laigter with synthetic luminance ramps (a +Y-facing surface → green 145 > 128;
  a +X-facing surface → red > 128) — no green-flip / DirectX conversion needed. The
  renderer consumes it directly. detail-blend / strength are Laigter-only knobs the
  marigold engine ignores.
- **depth** — the *shadowing* + AO + parallax input, but **NOT generated or
  distributed**. Depth is a deterministic integral of the normal (the normal *is* a
  height-field gradient), so baking + shipping a 16-bit map is redundant with the
  normal we already ship. The renderer **integrates it client-side** from the normal
  at load (Frankot–Chellappa / Poisson), caching the height texture like any other —
  a CPU FFT (rustfft in the WASM client) or a GPU Jacobi/multigrid solve in WebGL.
  Normal + depth then describe ONE surface for free, and there's nothing to keep in
  sync. `marigold/normal_depth.py` + `art depth` remain as the **reference
  implementation** for that client port and for eyeballing relief.

  **Caveat — relief ≠ standing height.** Integrating the normal recovers *surface
  relief* (bark ridges, branch tiers), never *world elevation* (that a conifer stands
  tall and casts a long ground shadow) — that was never in the normal. So client-side
  integration covers per-sprite **self-shadow / AO / parallax**. If **cross-object**
  shadows (tall tree over a short bush) are ever wanted, that standing height must be
  authored separately — a cheap per-sprite `height` scalar (e.g. alongside
  `&thing.size`), NOT a depth texture and NOT derivable from the normal.

### Normals vs shadows: the depth map

Normals and depth are complementary halves of 2.5D lighting: **normal → shading**
(how lit a surface is, N·L), **depth → shadows** (what blocks the light). A normal
map can't cast a shadow — that needs occlusion, i.e. a height field. Heightfield
ray-marching (horizon mapping) over a composited depth buffer gives dynamic
directional shadows with no 3D geometry.

> The caveats in the rest of this section (frame-edge flattening, billboard-depth ≠
> world elevation, per-image affine normalisation) are properties of the **monocular
> Marigold depth net** — now only the `art depth --engine marigold` path. The default
> **integrated** depth (from the normal) doesn't have the frame-edge or billboard
> problems: it reconstructs the relief the normal encodes, so a conifer's tiers come
> through directly. It's still per-sprite affine (normalised within the silhouette).

**Frame-edge flattening (handled).** Monocular depth flattens any object that runs
to the frame border — it can't tell the object doesn't continue past it. Dense
berry pieces on a square canvas (content touching the edge) came out as flat
near-blobs, while the same asset's sparser pieces (floating with margin) resolved
fine. Fix: `depth.py` pads each sprite off the edges by `--pad-frac` (default 0.5
of content size) per side, runs depth on the floated frame, then crops back —
verified to fix the whole berry set. This is separate from the camera-depth issue
below (padding restores the dome; it's still camera-depth, not standing height).

Two conventions, both settled at render time ("build the renderer to match"):

- **Affine-invariant, per sprite** — Marigold normalises depth per image (0 near,
  1 far, planes chosen per image). Fine for per-sprite AO/self-shadow; a shared
  world height buffer for cross-object shadows needs per-sprite scale/offset.
- **Billboard depth ≠ world elevation** — for an upright sprite (conifer), depth
  is camera-distance: the whole tree reads as one "near" plane vs a "far"
  background, *not* a tall-canopy→short-base gradient. So it's a coarse whole-sprite
  occluder, not rich per-pixel relief. Top-down terrain tiles (depth ≈ elevation)
  are the case where it's a genuine height field — spike those before leaning on it.

Depth has **no alpha** (16-bit grayscale) and the normal ships a **flat opaque
background** (#8080FF outside the silhouette) — neither carries coverage. Coverage
comes from the **albedo alpha** (the one map that keeps the silhouette); the renderer
masks normal/depth writes by albedo coverage, so the flat padding never composites
into a neighbour.

### Ambient occlusion

**We do not ship a baked AO or shading map today.** When AO is wanted, derive it
from the **depth** height field (a proper AO-from-height bake) — better than
Laigter's guess and cleaner than dividing the shading. Reasons we don't bake AO
from other sources:

- **Marigold shading carries directional light** (see the warm tint in a decomposed
  conifer). Baking it would fight the normal-driven lighting — the scene would be
  lit twice. So shading is *not* an occlusion channel and must not be shipped as
  one (`GRID_MAPS` already reserves `occlusion` for true AO — don't overload it).
- **Laigter's AO is a heightfield guess** with the same paint-edge weakness as its
  normals — not trustworthy enough to bake.

### Where AO comes from later

Preferred source is the **depth** height field: a standard AO-from-height bake
(horizon/hemisphere sampling of the heightmap). It's the same data that drives
shadows, and it's directionless by construction.

A second option, if depth proves too coarse on some art (billboards), is to derive
AO from Marigold's **shading**: estimate `L` from `shading` + `normal`, compute
`max(N·L, 0)`, take `AO ≈ shading / directional` — the directionless remainder is
the contact/crevice occlusion. Clean because it runs on the smooth shading channel
(no albedo edges to halo). Emit shading with `art delight <kind> --emit
albedo,albedo_shading` (off by default).

Either way it's a later polish pass — good normals + a screen-space/height AO pass
get most of the way first. Note the split of responsibilities: **directional
shadows come from depth at runtime** (can't be baked — they move with the light);
only the static AO subset is a candidate for baking.

## Empirical findings (the spike)

Run on the 2080 Ti against real masters:

- **wall_smooth** (monochrome grey): Marigold albedo is clean but nearly **flat
  white** — *correct*, because a grey wall's true base colour is neutral and all
  the tone is shading. All the form moved into the shading channel. Confirms that
  albedo-only rendering needs a lighting pass to look like anything.
- **conifer** (coloured/organic): the ideal case. Albedo is **flat, evenly lit,
  and keeps its colour** (green needles, brown trunk) with the directional shading
  removed and no halos/vignette/smear. The old divide, by contrast, barely changed
  the input. Residual caught the specular needle-tip highlights.
- **Residual** is minor for matte assets and has mild edge colour-fringing; it
  matters for glossy/emissive content, not stone or bark. This is exactly why it's
  now emitted (`N.albedo_residual.png`) and gated into the shipped emissive map —
  see below.

Takeaway: Marigold albedo is trustworthy on coloured content and a clear quality
win over the divide; on monochrome content it (correctly) goes flat, which is a
render-side problem, not a decomposition problem.

## The tooling

Scripts under `marigold/`. The model-backed ones load once and batch every
`*.diffuse.png` under the given paths, share `delight.py`'s discovery / path helpers,
a fixed `--seed` (2024, reproducible), and `--steps`/`--ensemble`/`--resolution`/
`--out-dir` knobs (small sprites are upscaled to the processing resolution internally
— lower it if the model hallucinates detail):

- **`delight.py`** — IID de-light → `N.albedo.png` (+ `albedo_shading`/
  `albedo_residual` via `--emit`; `maps` emits `albedo_residual` by default so
  `emissive` can gate it). Re-applies the sprite alpha (palette inputs normalised
  to RGBA first).
- **`bin/lib/emissive.py`** — gates + cleans `N.albedo_residual.png` into the
  shipped `N.emissive.png` (CPU, no venv/GPU): threshold the luminance, erode the
  coverage to drop the edge fringe, black elsewhere. Non-destructive (the residual
  is preserved), so `art emissive <kind>` re-tunes without re-running the de-light.
  Additive at render: `out = albedo·lighting + emissive`.
- **`normals.py`** — the OPTIONAL Marigold normal engine (`art normal --marigold`)
  → `N.normal.png` with a **flat OPAQUE background**: RGB outside the silhouette is
  filled with the flat facing-viewer normal `#8080FF` (0,0,1) and left opaque (no
  silhouette alpha; coverage comes from the albedo). Same `#8080FF` fill and same
  `--pad-frac` frame-edge padding as the default Laigter path.
- **`normal_depth.py`** — **reference** depth-from-normal integrator (not run by
  `maps`; depth ships nowhere — the client does this at render time). **No model/GPU**:
  integrates the co-located `N.normal.png` slope field into `N.depth.png` (Frankot–
  Chellappa FFT), forcing a flat plateau outside the silhouette (from the diffuse
  alpha). `--invert` for near=0/far=1, `--visualize` for `N.depth-viz.png`. This is
  the algorithm to port to the client (rustfft/WASM or a WebGL multigrid solve).
- **`depth.py`** — the monocular Marigold depth net (`art depth --engine marigold`),
  kept only for the rare photoreal-ish case → `N.depth.png` (no alpha; coverage from
  the shared mask). Flattens on flat art — that's why integration exists.
  `--visualize` writes a colour-mapped `N.depth-viz.png`.

- **`bin/marigold`** — venv wrapper mirroring `bin/laigter`. `build` creates the
  venv (prefers `uv`, so it needs neither `sudo` nor the system `python3-venv`),
  `config` reports torch/CUDA status; subcommands `delight` / `normals` / `depth` /
  `normal-depth`. First run downloads each checkpoint (~a few GB) to the HF cache.
- **`bin/art`** front-ends over a master dir: `art delight` / `art normal`
  (`--engine marigold|laigter`, default marigold) / `art depth`. Legacy divide
  kept as **`art delight-divide`**.
- **Wiring:** `art remaster` → `art split` → `art maps`, and `maps` runs all three
  (normal → albedo → depth). So `art remaster <kind>` (or `art maps <kind>`)
  produces the full Marigold set end-to-end. Grid categories (`linked`) skip only
  the per-piece **outline** (auto-tile seams) — they DO get maps now.

The `marigold/` code is tracked; the venv and HF cache are git-ignored
(`/marigold/.venv/`, `/marigold/.cache/`).

## GPU / environment

- **RTX 2080 Ti** (Turing, sm_75, 11 GB) via the WSL2 NVIDIA driver — **sufficient
  for inference**, no bigger GPU needed. fp16, ~3.6 s/sprite at steps=4/ensemble=5,
  ~8 GB free headroom. A larger/Ampere+ GPU would only matter for fine-tuning on
  our sprite style, high-res batches, or the heavier full-svBRDF models.
- **torch must match the driver.** The box runs a CUDA 12.9 driver; the default
  PyPI wheel targets cu13x and fails ("driver too old", `cuda=False`). `bin/marigold
  build` pins a **cu124** torch (2.6.x, supports Turing). Override for a different
  driver with `bin/marigold build --torch-index https://download.pytorch.org/whl/cuXXX`.

## Open work

1. **Renderer lighting pass — the blocker.** [AlbedoMap](../pixijs/src/game/viewport/AlbedoMap.ts)
   bakes albedo only; nothing consumes the normal or depth. Until a lighting pass
   exists, flat albedo has nothing to restore its form. This — not another map — is
   what stands between "flat albedo on screen" and a lit, dimensional world.
   Sketch: bake normal (and depth) into companion buffers alongside albedo, then a
   shader combines `albedo × lighting(normal, lights)` plus heightfield-raymarched
   shadows from the depth buffer. Settle the normal/depth conventions here (view-
   space normals read directly; depth height meaning per art class).
2. **Regenerate masters.** Spikes ran into scratch; on-disk masters still carry the
   old divide albedo. Run `art maps <kind>` (or a full `art remaster`) to switch
   them over — do this *after* the lighting pass exists, so flat albedo isn't a
   visible regression in the meantime.
3. **Derived AO** (optional, later) — the shading-divide recipe above, if
   screen-space AO + normals aren't enough.
4. **Validate on more AI-sprite styles.** Marigold is trained on photoreal/Hypersim
   data; stylised input is its weak spot. Spot-check new asset styles before trusting
   the albedo blindly.

[mg]: https://huggingface.co/docs/diffusers/using-diffusers/marigold_usage
