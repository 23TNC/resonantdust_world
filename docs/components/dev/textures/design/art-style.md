# Art style

The canonical description of Resonant Dust's visual style. Read this before
generating, editing, or reasoning about any sprite/texture art.

## Reference & feel

**RimWorld / Prison Architect.** Flat, simple, hand-painted 2D game art:

- Compact shapes, soft irregular silhouettes, a slightly hand-painted look.
- **Minimal / cel shading** — limited flat tones, no photoreal rendering, no
  heavy gradients, no realistic 3D lighting baked into the diffuse.
- Clean **bold outlines** around subjects.
- Simple, readable at small on-screen sizes.

This style is native to **SDXL** (see [sprite generation](#sprite-generation)).
Photoreal models (FLUX-base) fight it by over-rendering detail — avoid them for
base sprite generation.

## Perspective — oblique 3/4 top-down (pseudo-isometric)

**This is the single most important and most misunderstood point.**

The **world grid is viewed top-down**, but **objects and characters are drawn in
an oblique three-quarter top-down perspective** — they show their **front or
sides**, NOT a true bird's-eye projection, and NOT true isometric geometry. Often
described as *3/4 top-down* or *pseudo-isometric*.

Do **not** render characters/creatures as if seen from directly overhead. A
straight bird's-eye animal looks wrong and is out of the model's training
distribution.

### Directional facings (pawns / creatures)

Pawns and creatures need directional sprites. Each direction is a specific
**camera view of the subject**, not a rotation of a top-down blob:

| Facing | View of the subject |
| --- | --- |
| **East** | **Side profile**, facing right |
| **West** | **Side profile**, facing left (horizontal mirror of East) |
| **South** | **Front view** — subject faces the viewer/camera |
| **North** | **Back view** — subject faces away, back/rump/tail toward camera |

Practical consequences for generation:

- **East/West are easy** — side profiles are in-distribution; SDXL produces them
  cleanly from a prompt. West = mirror of East, so only one side needs generating.
- **South/North are hard** — SDXL has a strong "animal = side view" prior and
  resists turning the subject to face the camera or show its back. Prompt-only is
  a coin-flip. These need an **edit model** (Qwen-Image-Edit / FLUX-Kontext) to
  rotate one good side sprite to front/back, or an *organic* front/back ControlNet
  reference (never crude geometric templates — those get traced into a blob).

## Color / tint regions — channel-packed albedo

Sprites use three isolable tint regions so one texture recolors to any palette:

- **primary** — the largest main body/material area (e.g. main fur).
- **secondary** — larger markings, supporting materials, contrasting fur.
- **detail** — small accents: eyes, trim, minor markings.

**Packing:** the final **albedo packs the three regions into R / G / B as
grayscale shading masks** — R = primary, G = secondary, B = detail. Each channel
stores *shading* (light→dark), NOT color, and is zero outside its region.

**Render:** each channel is treated as grayscale and multiplied by a **tint**
color, then summed: `out = R·tintPrimary + G·tintSecondary + B·tintDetail`. So one
albedo → unlimited palettes. E.g. tint(R→white, G→gray, B→blue) = white/gray wolf
with blue eyes; tint(R→near-black, G→white, B→green) = black/white wolf, green
eyes. (Pure-black tint loses shading — tint toward dark-gray, not 0,0,0.)

**Generation implication:** stage 1 must produce cleanly **separable** regions.
Relying on the model to paint separable colors + auto-segmentation is fragile
(esp. eyes). Preferred: the template carries an authored **region map** (from the
`<id>.<dir>.<layer>.png` parts / PSD layers); i2i generates *shading*
pose-locked to the template, the aligned region map assigns each pixel to its
channel, and packing is exact. Per region, stretch luminance to full range so
tints read bright; near-black outline pixels stay dark under any tint.

## Background & cutout

Sprites are delivered as **transparent alpha cutouts**.

- The legacy magenta `0xFF00FF` background existed only as a chroma key so sprites
  could be cut out. It is **retired** for generated art: `rembg` produces a clean
  alpha cutout directly, so there is no need to render (then re-key) a magenta
  field. `bin/art` blob-detects on alpha, so alpha-in is exactly what it wants.
- Keep backgrounds flat and clean during generation; the cutout removes them.

## Pipeline pointers

- **Prompt templates:** `docs/prompts/sprite.{east,south,north}.txt` (hand-authored;
  the ComfyUI generation pipeline is replacing them — see `dev/scripts/art/plan`).
- **Sprite generation** (in progress): ComfyUI-driven, SDXL + ControlNet + rembg.
  See the `sprite-gen-pipeline` memory for the working recipe and status.
- **Texture pipeline:** `bin/art` slices/keys/masters sprites; masters normalize
  to power-of-two sizes. `bin/marigold` derives albedo/normal/depth maps for the
  renderer's lighting pass. See `docs/de-lighting.md`.
