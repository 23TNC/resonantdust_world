# Forks — beat e07

_A choice I resolved, with what was rejected and why. A fork is mine; a [blocker](blockers.md) is
the user's._

## F1 — The shape of the selection loop {#f1}
_2026-08-02 · resolved at plan time · **the user picks the EPOCH from per-epoch samples; no
breed-from-the-winner**_

**Chosen.** Sample every epoch on a pinned prompt/seed set; the user picks the best **epoch**.

**What the user proposed**, and why the mechanism doesn't hold: "run 10 generations, I select the
best, we then build another 10 off of that." Two readings, both problematic.

- *If the ten are epochs of one run* — picking the best and continuing from it is just training
  longer. Epoch 7 → 8 is what training already does; the selection adds no information.
- *If the ten are configs* — forking from the winner's WEIGHTS and training ten more compounds
  overfitting. Run-4 was already 15 epochs on 459 images (765 effective); continuing to 25+
  memorises.

Genetic search doesn't map onto gradient descent: there is no crossover between two LoRAs, and
"more training from the winner" is not mutation, it is the same descent going further.

**Kept from the proposal, because it is the right instinct:** the user's eyes decide, samples come
out every epoch rather than at the end, and no automated score ships a model on its own. `e07`'s
own epoch-7 provenance is the evidence that the best checkpoint is mid-run.

## F2 — Attribution is NOT a goal of this stream {#f2}
_2026-08-02 · **RE-RESOLVED on the user's instruction** — the original planned a single-variable
decomposition · **change everything at once and judge the output**_

**Chosen.** Change base model, dataset, precision, batch and rank together, and judge the result
by the sprites. Do not run controlled variants.

**The user's instruction, verbatim:** *"we are **not intending to create a valid comparison**, we
are intending to create the **best lora we can** utilizing lessons learned from our previous runs
and advantages of the new hardware."*

**Why it is the right call here.** Attribution costs runs, and runs cost 2–4 h each. The prior
streams already bought the lessons worth having — `iou_ref` agrees with the eye, ESRGAN beats
LANCZOS, pinned `fill` correlates with every failure, the config was VRAM-capped. Spending 25+
GPU-hours to re-derive *which* of those matters buys knowledge, not sprites, and the user wants
sprites. Bisection stays available later, on a model worth bisecting.

**The cost, accepted explicitly and recorded so nobody re-litigates it:** if the result is worse,
we will not know which change did it. That is the trade, not an oversight.

**What the original fork said**, kept because it was sound reasoning for a different goal: `e07` is
the only model trained on dataset v1 and the only winner; runs 3/4/5 are the only ones on v2 and
the only losers; P2 bundled three changes (upscaler, scale normalisation, 768→1024) that no run has
isolated ([I3](issues.md#i3)). A future stream that needs *why* starts there.

## F3 — What "the user selects" is allowed to select {#f3}
_2026-08-02 · resolved at plan time · **the eye ratifies; `iou_ref` is the tiebreak**_

**Chosen.** Every A/B reports gate + `iou_ref` + signed aspect AND emits the visual sheet, and no
verdict is accepted before the images are looked at. Where they disagree, the images win and the
disagreement is recorded.

**Why not "the metrics decide".** Four times in this project the images overturned the numbers —
the anteater (the metric rated the visually best output worst), run-4 south (passed 10/18 while
broken), run-4 east (a tie while the pose convention broke), and run-4 overall (a "slightly worse"
aggregate hiding "better on east, broken on south").

**Why not "the eye alone".** `sprite-eval-trust` spent a whole stream making the ruler agree with
the eye and succeeded — `iou_ref` called both run-4 failures 6/6. It also survives a base-model
change, because it scores the generated silhouette against the **real corpus sprite** and never
looks at the model that produced it. That is what lets `e07`'s scoreline stay a valid bar here.

## F4 — Which base model {#f4}
_2026-08-02 · resolved at plan time · **Illustrious-XL v1.0, pending the P1 probe**_

**Chosen (provisionally).** Illustrious-XL v1.0, with `animagine-xl-4.0` as the alternate and
`sd_xl_base_1.0` as a clean reference. [P1](todo.md) confirms or overturns it by generating,
because this pick is reasoning and the project's record strongly favours measurement.

**Why any change at all.** `generate.py` names `sdxl/cyberrealisticXL_v80.safetensors`, and before
2026-07-28 the box held no other SDXL checkpoint — so every run including `e07` was fitted onto a
**photorealism finetune** while targeting flat regions bounded by hard black outlines. The LoRA has
been spending capacity fighting its own base.

**Why Illustrious over Animagine.** Both are Danbooru-tag bases with strong flat-colour and
hard-outline priors, which is the property we want. Illustrious has the better prompt adherence and
is the more common foundation for downstream LoRA training; Animagine 4.0 is tuned more narrowly to
anime character portraiture. Our subjects are quadrupeds in an oblique game-sprite convention —
off-distribution for both, since Danbooru is overwhelmingly human characters — so the more
*steerable* base is the better bet.

**Stated plainly: both are anime bases and our target is not anime.** What we are buying is the
flat/outline prior, not the style. The risk is a pull toward anime faces and proportions on
animals, which the P1 probe will show before a single GPU-hour is spent on it.

**Rejected — `sd_xl_base_1.0`.** Neutral rather than helpful; it has no flat-art prior to lend, so
it asks the LoRA to teach the whole convention from 459 images again.

**Rejected — stay on cyberrealisticXL.** It is the incumbent only because it was the only SDXL
checkpoint on the box in March, not because anything chose it for this art.

**Rejected — a non-SDXL base (Flux, SD3.5).** It would strand the SDXL ControlNets already on the
box (`mistoline-lineart`, `controlnet-union-promax`), the whole two-stage architecture depends on
ControlNet, and the licensing question the user raised on 2026-07-28 is unresolved for Flux.

## F5 — Should ComfyUI autostart on the unraid box? {#f5}
_2026-08-02 · resolved at P0.2 · **no — fix the silent fallback instead**_

**Chosen.** Leave ComfyUI off the unraid autostart list with restart policy `no`, exactly as it is
today. Start it on demand.

**Why the question came up at all:** [I2](issues.md#i2) — `prep_train` silently swaps ESRGAN for
LANCZOS when the box is unreachable, so "ComfyUI happened to be down" quietly becomes "we trained
on a different dataset". Autostart looked like insurance against that.

**Why it is the wrong insurance.** It papers over the failure rather than removing it. [P0.3](todo.md)
makes the fallback *loud*, after which a down ComfyUI is an immediate non-zero exit instead of a
corrupted corpus — which is the actual fix, and it holds whether or not the service happens to be
running. Once that lands, autostart buys convenience only.

**And the convenience is worth less than it looks.** Measured today: the container takes **~10
minutes** to serve, nearly all of it a recursive `chown` over the mounted `ComfyUI`/`HF`/`venv`
trees. That is paid on every boot, for a GPU service used in bursts.

**Whose machine it is.** The box's standing job is home automation — Home Assistant, NodeRed and
zwave-js-ui are the three on the autostart list, and they are what the house depends on. ComfyUI is
a heavy occasional tenant. Changing how a home server boots is not something this stream needs, and
the current configuration is the user's existing choice; nothing here requires overriding it.

**Rejected — autostart with `restart: unless-stopped`.** Same objection, plus it would bring the
service back after a deliberate stop, which is the opposite of what an occasional tenant wants.

**Revisit if** the art pipeline becomes a scheduled/unattended job. A cron-driven rebuild that
cannot ask a human to start a container is a real reason; "it is convenient" is not.

### F4 — MEASURED at P1.1: the probe overturns the pick
_2026-08-02 · **Animagine, not Illustrious** — pending the user's confirmation from the sheet_

My pre-measurement reasoning above chose Illustrious on prompt adherence and its LoRA-training
ecosystem. **The images say the opposite**, and on the one axis that matters most for this art:

| | east cells wider than tall | mean aspect (east) | read |
|---|---|---|---|
| Illustrious-XL v1.0 | **0 / 4** | **1.02** | square/upright — emblems, faces, logo compositions |
| animagine-xl-4.0 | **4 / 4** | **1.89** | side-profile bodies with quadruped proportions |

Aspect on an east view is exactly the body-versus-bust discriminator `sprite-eval-trust` built its
gate around, and the separation is not marginal. Visually the sheet agrees: Animagine draws
full-body quadrupeds in side profile with flat fills and heavy black outlines on wolf/tiger/fox/pig;
Illustrious draws angular emblems, a stacked pile of foxes, and four pigs.

**What stands from the original reasoning:** both are Danbooru bases, both give the flat-colour
hard-outline prior we wanted, and both are wrong on front views ([I7](issues.md#i7)).

**What does not:** "Illustrious is the more steerable base" was an inference from its ecosystem
reputation, not from this art. For quadrupeds in side profile it is measurably the worse prior.

Caveat kept honest: 8 cells, one seed, one heavily-loaded prompt, and an untrained base's prior
only *correlates* with what it will learn — it does not settle it. This is enough to choose which
to spend a training run on; it is not proof.

## F6 — Training resolution {#f6}
_2026-08-02 · resolved at P2.3 · **1024²**_

**Chosen.** 1024², the SDXL native resolution both candidate bases were trained at.

**Why.** 768 entered this project as a VRAM accommodation, not a decision — run-4 held 10,379 MiB
of the 2080 Ti's 11,264 ([I4](issues.md#i4)). That ceiling is gone. Training a 1024-native base at
768 asks it to work off-distribution for no remaining reason.

**Why it is not "the P2 change that broke things".** It might be — 768→1024 is one of the three
bundled P2 changes nobody isolated ([I3](issues.md#i3)). But `e07`'s base was cyberrealisticXL,
which is also SDXL-native-1024, so 768 was off-distribution for *it* too, and it still won. That
makes resolution the least likely of the three to be the culprit, and [F2](forks.md#f2) says build
the best thing rather than litigate it.

**Rejected — 768 "because e07 used it".** Copying the winner's incidental constraint is
cargo-culting a limitation we no longer have.
