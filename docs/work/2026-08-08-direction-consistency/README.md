# Visual consistency across the three views — 2026-08-08

_Component: [`dev/art`](../../components/dev/) · `bin/lib/generate.py` + the ComfyUI graph it drives.
Plan in [`todo.md`](todo.md); decisions in [`forks.md`](forks.md); findings in [`issues.md`](issues.md);
the LoRAs this stream renders with in [`checkpoints.md`](checkpoints.md). Successor to
[`2026-08-06-lora-style-not-species`](../2026-08-06-lora-style-not-species/README.md), which produced
`rd_diremph_anima_r20_g07` — the first LoRA to render a usable e/s/n wolf set through the real
pipeline._

## The measurement this stream rests on

The corpus holds three views of the same animal. Their **luminance barely moves between views**:

| corpus species | east | south | north | **e↔n spread** |
|---|---|---|---|---|
| Wolf_Timber | 90.7 | 84.5 | 83.0 | **7.7** |
| Direwolf | 69.6 | 61.9 | 63.8 | **7.7** |
| AEXP_Coyote | 120.2 | 118.4 | 121.4 | **3.0** |
| Fox_Red | 88.7 | 81.7 | 90.7 | **9.0** |

Our generated wolf, `r20g07` through the pipeline at `cn 0.5 / cn-end 0.9 / strength 0.7`:

```
east  lum 133.9      south lum 146.5      north lum 158.8
                                        spread = 24.9
```

**24.9 against a corpus ceiling of 9.0 — 2.8× the worst real animal**, and it is not noise: the
brightening is **monotonic in generation order**, e → s → n, which is exactly the order the
pipeline renders them in. Two more defects fall out of the same measurement:

- **Absolute value is ~50 points too pale.** Corpus animals sit at 62–121; ours at 134–159.
- **Saturation has collapsed.** A grey corpus wolf measures 10.6–17.5; ours measures 4.1–6.4 —
  flatter than the greyest real wolf in the set.

## Why it drifts — the mechanism, read out of the code

[`generate.py`](../../../bin/lib/generate.py) renders **east first as a "hero"** (`graph_hero`,
txt2img + ControlNet), then renders south and north through `graph_ip`, which anchors each on the
east image via IP-Adapter at `IP_WEIGHT = 0.6`. Two consequences, both load-bearing:

1. **South and north never see each other.** Both hang off east independently, so their errors are
   free to diverge in opposite directions — and they do.
2. **The anchor is `weight_type: "style transfer"`**, which by construction discards composition and
   content and carries *style*. It is the wrong lever for "this is the same wolf": identity is
   precisely the content we are throwing away.

## The user's read, taken seriously

> *"I suspect we are hard locked to our control net."*

Correct, and worth stating plainly because it bounds what this stream can fix. At `cn 0.5 /
cn_end 0.9` the **silhouette comes from the authored template**, not from the model. So:

- **Outline consistency is already solved, and not by us** — it is inherited from the authored
  `template.{e,s,n}.0.png` triple. That is why the sprites line up in shape and disagree in colour.
- **Every new creature needs an authored template triple.** That dependency is real and this stream
  does **not** remove it — that is the [`lineart-lora`](../2026-07-27-lineart-lora/README.md)
  stream's job (generate the line drawing, then use it as the ControlNet source).
- **What is left free is the interior**: value, saturation, markings, palette. That is where all
  24.9 points of drift live, and it is what this stream targets.

## What the 3090 makes available that the 2080 Ti did not

Verified present on the box today, not assumed:

| capability | what is installed |
|---|---|
| **IP-Adapter with content weight types** | `comfyui_ipadapter_plus`, `ip-adapter-plus_sdxl_vit-h`, `clip-vit-h-14-laion2B` — `IPAdapterAdvanced` exposes composition-carrying weight types, not just `style transfer` |
| **Split style from composition** | `IPAdapterStyleComposition` takes a style reference *and* a composition reference as separate inputs |
| **Many references at once** | `IPAdapterBatch`, `IPAdapterEncoder`, `IPAdapterCombineEmbeds` — a view can be anchored on *both* others rather than on east alone |
| **Union control** | `controlnet-union-promax` + `SetUnionControlNetType` — several control signals in one pass |

**Correction, 2026-08-08 ([I9](issues.md#i9)).** This section originally called the batched pass
"the single most promising" capability, on the reasoning that "three views denoised together share
one noise schedule and one prompt evaluation, which is the standard way consistency is bought."
**That is wrong for our case.** It holds only when batch items share conditioning, and ours cannot:
each direction needs its own prompt *and* its own ControlNet template, while a `KSampler` broadcasts
one conditioning and `ControlNetApplyAdvanced` one image across the whole batch. Three views with
three prompts and three controls are three independent generations sharing a sampler call — that is
throughput, not consistency. The mechanism that *would* share information across views is attention
sharing, and no SDXL `ReferenceOnlySimple` exists on this box; every reference-attention node
present belongs to an architecture we are not running.

**What the 24 GB actually buys, corrected:** IP-Adapter *and* ControlNet *and* several reference
images resident at once — which is [P2](todo.md). True reference attention is a property of the
**edit-model architecture** ([P6](todo.md)/[F5](forks.md#f5)), where `ReferenceLatent` is precisely
how Qwen-Image-Edit conditions on a source image. The hardware claim was right; the mechanism was
not.

## Design stance

- **The defect is measurable, so every change is judged by the number.** Cross-direction luminance
  spread against the corpus ceiling of 9.0, reported next to the sheet, every time.
- **The eye ratifies; the metric is the tiebreak.** Carried unchanged from
  [`beat-e07` F3](../2026-08-02-lora-beat-e07/forks.md#f3) — four times in this project the images
  overturned the numbers.
- **Do not re-open the LoRA.** `r20g07` is the input to this stream, not its subject. Training is
  settled ([checkpoints.md](checkpoints.md)); this is a *pipeline* stream.
- **Prefer the cheap lever first.** The prompt-form fix ([I1](issues.md#i1)) costs one line and is
  already evidenced; batching costs a graph rewrite. Order by cost, not by novelty.
- **Say plainly if the spread does not close.** A pipeline that cannot hold identity across three
  views is a result worth having.

## Future intent this plan must not trim

- **The template dependency is NOT in scope and must not be quietly "solved" by weakening `cn`.**
  Dropping control strength would close the value gap by letting the model repaint freely, and it
  would take the silhouette with it — the exact regression
  [beat-e07 P4](../2026-08-02-lora-beat-e07/todo.md) already paid to learn. If a change needs `cn`
  below 0.5, it is a different stream.
- **The two-stage architecture stays the destination** — generate a line drawing, use it as the
  ControlNet source, render detail in a second pass. Whatever consistency machinery lands here must
  survive the template becoming *generated* rather than authored.
- **Hue is discarded downstream** — the game re-tints via layer maps and only **brightness**
  survives. That is *why* luminance is the metric and saturation is secondary; it is not a licence
  to ignore saturation, because the layer-map decomposition still has to find material regions.
- **`part` is already a first-class axis** (`--part`, body=0 head=1). Multi-part creatures multiply
  the consistency problem rather than changing it; nothing here may assume one part per creature.
