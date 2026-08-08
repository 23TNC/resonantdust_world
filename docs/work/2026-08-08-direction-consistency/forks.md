# Forks — direction consistency

_A choice I resolved, with what was rejected and why. A fork is mine; a
[blocker](blockers.md) is the user's._

## F1 — This is a PIPELINE stream, not a training stream {#f1}
_2026-08-08 · resolved at plan time · **the LoRA is frozen at `r20g07`**_

**Chosen.** `rd_diremph_anima_r20_g07` is an input, not a subject. No training run is planned, and
a consistency failure is not grounds to open one.

**Why.** The predecessor spent ~25 GPU-hours over eleven runs to reach a checkpoint that renders a
usable three-view set. The drift measured here is **24.9 luminance points between views of one
animal** produced by a pipeline that renders those views in three separate sampler calls with a
content-discarding anchor ([I2](issues.md#i2)/[I3](issues.md#i3)). That is a compositing problem
with a mechanism we can point at, and reaching for another training run would be answering it with
the most expensive tool available and the least evidence.

**Rejected — train a direction-consistency LoRA.** There is nothing to train it on: the corpus's
three views per species are *already* consistent (spread ≤ 9.0), so the model has seen the right
thing 701 times. The information is not missing from training; it is being discarded at inference.

**Rejected — go back and pick a different generation.** g07 and g08 are both retained
([checkpoints.md](checkpoints.md)) and either can be swapped in as a one-line change, but choosing
between them before the ruler exists ([P0](todo.md)) would be picking by eye on the exact axis the
eye is worst at.

## F2 — Cheapest lever first: prompt form, then anchoring, then batching {#f2}
_2026-08-08 · resolved at plan time · **ordered by cost, not by novelty**_

**Chosen.** [P1](todo.md) (prompt form) → [P2](todo.md) (anchoring) → [P3](todo.md) (batched pass).

The temptation is to open with the batched sampler pass, because it is the capability the new
hardware unlocks and it is the most interesting. It is also a graph rewrite. Meanwhile the prompt
form is **one constant** and already has evidence behind it — the prose form's saturation runs
48/20/28 across three views of one animal against the tag form's 4.4/6.4/4.1
([I1](issues.md#i1)). Shipping the interesting change first would fold that free correction into a
big diff and make the attribution unreadable.

**The risk, accepted:** P1 and P2 may close the spread enough that P3 never gets a fair trial, and
the 24 GB capability stays untested. Recorded so that outcome is a decision rather than a drift —
if P1/P2 clear the ceiling, P3 still runs once as a measurement.

## F3 — `cn` stays at 0.5; consistency is not bought by weakening control {#f3}
_2026-08-08 · resolved at plan time · **a hard floor, not a default**_

**Chosen.** `cn 0.5 / cn-end 0.9` is frozen for the stream. Any change that needs `cn` below 0.5
is out of scope and belongs to a different stream.

**Why it needs saying.** Lowering `cn` *would* reduce the measured drift, by letting the model
repaint each view freely toward whatever it thinks a wolf looks like — and it would take the
silhouette with it. [beat-e07 P4](../2026-08-02-lora-beat-e07/todo.md) already bought that lesson:
at `cn 0.2` the east sprite came out **a featureless white slab with a head stuck on**. A metric
that improves while the sprite gets worse is the failure mode this project has hit four times, and
`cn` is the single knob most able to produce it here.

**Consequence, stated so it is not mistaken for an oversight:** silhouette consistency is inherited
from the authored templates and is **not evidence about this stream's changes**
([I4](issues.md#i4)). Only interior metrics count.

## F4 — Luminance spread is the primary metric; saturation is secondary but not ignorable {#f4}
_2026-08-08 · resolved at plan time_

**Chosen.** Cross-direction **luminance spread**, against the corpus ceiling of **9.0**, is the
number every change is judged by. Saturation and absolute value are reported alongside and neither
may regress.

**Why luminance leads.** The game re-tints through layer maps, and only **brightness** survives
downstream — settled in the predecessor and unchanged. A hue that drifts between views is largely
cosmetic in the shipped asset; a *value* that drifts is visible in the game.

**Why saturation is still reported.** The layer-map decomposition finds material regions by colour
clustering. A fully desaturated sprite is not merely dull, it is **harder to decompose** — and ours
already measures 4.1–6.4 against 10.6–17.5 for a grey corpus wolf. [P4](todo.md) checks the
decomposition explicitly rather than assuming it survives.

**Why absolute value is tracked separately from spread.** Three views that agree with each other at
150 and disagree with the corpus at 87 would score perfectly on the primary metric while being
uniformly wrong. Consistency and correctness are different properties and the plan measures both.

## F5 — The option survey the plan should have opened with {#f5}
_2026-08-08 · raised by the user, resolved after surveying the box · **Qwen-Image-Edit is the one
worth the download; SDXL+IP-Adapter stays the production default**_

Prompted by [D1](deviations.md#d1). Every node class below is **already exposed by ComfyUI on the
box** — verified against `/object_info`, not assumed. Only the *weights* are missing, so the cost
of each option is a download, not an integration.

| option | ComfyUI nodes present | licence | fit |
|---|---|---|---|
| **Qwen-Image-Edit** | `ModelMergeQwenImage`, `QwenImageDiffsynthControlnet`, `EmptyQwenImageLayeredLatentImage` | **Apache 2.0** | purpose-built view-change with identity preservation; VAE + a Qwen text encoder already staged |
| **Hunyuan3D v2** | `EmptyLatentHunyuan3Dv2`, `Hunyuan3Dv2ConditioningMultiView` | Tencent community (restricted) | image→mesh, then render 3 cameras — consistency *by construction* |
| **Stable Zero123** | `StableZero123_Conditioning_Batched`, `StableZero123_BatchSchedule` | non-commercial research | multi-view from one view; exactly our problem shape |
| **SV3D** | `SV3D_Conditioning`, `SV3D_BatchSchedule` | Stability non-commercial | orbit around an object |
| **FLUX.1 Kontext** | `FluxKontextImageScale`, `FluxKontextMultiReferenceLatentMethod` | **non-commercial** | strong editor; `vae/ae.safetensors` already staged |

**Chosen — add Qwen-Image-Edit as a parallel track (P6); keep P0–P4 as the production path.**

**Why Qwen over the rest.** It is the only option that is simultaneously (a) the design doc's own
first choice, (b) **Apache 2.0**, and (c) already half-staged. Licence is the sharp filter here and
it eliminates most of the field: this is a game we intend to ship, and FLUX Kontext dev, SV3D and
Stable Zero123 are all non-commercial. Their being technically excellent does not make them usable,
and the licensing question the user raised on 2026-07-28 is still unresolved
([beat-e07 F4](../2026-08-02-lora-beat-e07/forks.md#f4)) — this fork does not resolve it, it routes
around it.

**Why the 3D route is genuinely interesting and still not first.** Rendering three cameras from one
mesh makes the views consistent *by construction* rather than by coaxing, and it would also
generate the template triple — attacking the "hard locked to our control net" dependency the user
named ([I4](issues.md#i4)) rather than working inside it. Held back because Hunyuan3D's licence is
restricted, the MIT alternative (TRELLIS) has no native node and needs a custom-node install, and a
mesh→render path produces *shaded 3D renders*, which is the opposite of the flat unshaded
convention ([art-style.md](../../components/dev/textures/design/art-style.md)). It is a bigger bet
than this stream should make on its own; recorded as future intent, not scheduled.

**Why P0–P4 are not abandoned.** The ruler ([P0](todo.md)) measures interior luminance spread and
is **architecture-independent** — it scores a Qwen output exactly as well as an SDXL one, which is
the property that makes the comparison in P6 possible at all. And the design doc names
IP-Adapter + ControlNet as *"the robust production default"* even while leaning on the edit model;
building the default is not wasted work if the edit model wins.

**What would overturn this.** If P6 shows Qwen-Image-Edit holds identity across e/s/n at a spread
inside the corpus ceiling while SDXL+IP-Adapter cannot, the edit model becomes the S/N mechanism
and P2's anchoring work is superseded. That is an acceptable outcome and the reason P6 runs at all.
