# Marigold-IID de-lighting

Learned de-lighting for master sprites, replacing the hand-rolled two-pass divide
(now `bin/art delight-divide`). Uses [Marigold Intrinsic Image Decomposition][mg]
— the **lighting** checkpoint `prs-eth/marigold-iid-lighting-v1-1` — which splits
each diffuse sprite as

    I = A * S + R

- **A — Albedo**: base colour, de-lit. Shipped as `N.albedo.png` (drop-in for the
  old albedo). This is what the renderer bakes today.
- **S — diffuse Shading**: the removed lighting. Emit for inspection / a future
  occlusion-style channel.
- **R — non-diffuse Residual**: additive highlights (painted rim lights,
  speculars) that a *division* can't remove. Emit for a future emissive/specular
  channel.

Because it's a learned decomposition it needs **no normal map** (unlike the
divide, which measured `N·L`), so `bin/art maps` now runs normal and albedo
independently.

## Setup (one-time)

```
bin/marigold build          # creates marigold/.venv, installs torch + diffusers
bin/marigold config         # prints paths + torch/CUDA status
```

The 2080 Ti (Turing, sm_75) is driven directly through the WSL2 NVIDIA driver in
fp16 — no Docker. First run downloads the checkpoint (~a few GB) to the HF cache.
If `build` pulls a CPU-only torch, rebuild with a CUDA wheel index:

```
bin/marigold build --torch-index https://download.pytorch.org/whl/cu124
```

## Use

Batch a whole object via the art front-end (writes `N.albedo.png` co-located):

```
bin/art delight wall_smooth
```

Or drive it directly for a spike/compare (all three targets, off to the side):

```
bin/marigold delight textures/master/linked.0/wall_smooth.0 \
    --emit albedo,shading,residual --out-dir /tmp/spike
```

Runs offline at master-generation time only; not in the game loop. Seed is fixed
(`--seed`, default 2024) so masters are reproducible. Tuning knobs: `--steps`
(default 4), `--ensemble` (default 5, precision vs. speed), `--resolution`
(default 768; small sprites are upscaled to this internally — lower it if the
model hallucinates detail).

[mg]: https://huggingface.co/docs/diffusers/using-diffusers/marigold_usage
