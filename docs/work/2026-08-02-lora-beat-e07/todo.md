# Beat e07 — plan

Items are checkboxes; tick in place (`[x]`), never move them. One action + acceptance each.
Ordering: **rig up**, **pick the base**, **build the dataset from lessons**, **train on the 3090's
headroom**, **the user selects**. This stream changes many variables at once on purpose
([F2](forks.md#f2)) — do not re-introduce single-variable runs to "make it comparable".

## P0 — Rig, and the bar to clear

- [x] Bring ComfyUI up on the 3090 and confirm torch sees the card. Acceptance: `/system_stats` answers on `:8188` reporting the 3090 and 24 GB.
- [x] Decide whether ComfyUI should autostart and record the choice. Acceptance: a line in `forks.md`; today it is absent from the unraid autostart list with restart policy `no`.
- [x] Make `prep_train` fail loudly when ComfyUI is unreachable instead of silently using LANCZOS. Acceptance: with the box down a rebuild exits non-zero; ESRGAN is opt-out via an explicit flag ([I2](issues.md#i2)).
- [x] Pin the sample set — fixed prompts, seeds and species — in a file every run and A/B reads. Acceptance: two invocations on one model produce byte-identical sample sheets.
- [x] CUT on the user's instruction — do not run the old model to "establish a bar" before training. Acceptance: the item is recorded as a plan error in `deviations.md`; `e07` is compared against only at [P4](todo.md), if at all.

## P1 — Choose the base model

- [x] Generate the pinned set on Illustrious-XL v1.0 and animagine-xl-4.0 with NO LoRA. Acceptance: one sheet per candidate, same prompts and seeds. cyberrealisticXL is NOT probed — it is being replaced, not evaluated.
- [x] Have the user pick the base whose prior sits closest to flat-region/hard-outline art. Acceptance: a base chosen, with the user's reasoning recorded in `completed.md` ([F4](forks.md#f4) holds my pre-measurement pick).
- [ ] Confirm the chosen base loads with the SDXL ControlNets already on the box. Acceptance: `mistoline-lineart` and `controlnet-union-promax` both produce a controlled generation without a shape or dtype error.
- [ ] Confirm which VAE the chosen base wants and wire it explicitly. Acceptance: a generation with no washed-out or artefacted output, and the VAE named in `completed.md` rather than left implicit.
- [ ] Point `generate.py`'s `MODEL` at the chosen base. Acceptance: the default no longer names a photorealism finetune; the old value is recorded in `completed.md` for rollback.

## P2 — Rebuild the dataset from the lessons

- [x] Restore v1-range scale variance in `prep_train`, targeting `e07`'s measured spread rather than run-5's timid jitter. Acceptance: fill sd near **0.148**, not 0.027 ([I1](issues.md#i1)).
- [x] Keep ESRGAN upscaling and re-verify it on the rebuilt set. Acceptance: outline sharpness near the measured 90.1, versus 43.1 for LANCZOS, over a 120-image sample.
- [x] Choose and record the training resolution for the new base. Acceptance: a resolution with a stated reason; 1024² is SDXL-native but was never separated from the other P2 changes.
- [ ] Rebuild the training set end to end and measure it before any training starts. Acceptance: image/caption counts match, fill sd and outline sharpness recorded, no silent-no-op ([I3](issues.md#i3)).
- [ ] Spot-check ~10 rebuilt images by eye against their sources. Acceptance: no clipping, no halo, no aspect distortion — the check that caught the P2 no-op.

## P3 — Train on the 3090's headroom

- [ ] Probe peak VRAM for a candidate config before committing to a full run. Acceptance: a MiB figure against 24576, so batch and rank are sized on data rather than guesswork ([I4](issues.md#i4)).
- [ ] Train in bf16 rather than fp16. Acceptance: the run completes with a loss curve free of the scaling spikes bf16 exists to remove; Ampere supports it and Turing had none.
- [ ] Replace grad-accum 4 with a real batch that fits 24 GB. Acceptance: the effective batch is stated and peak VRAM stays under ~22 GB with headroom for sampling.
- [ ] Raise `dim`/`alpha` past the 11 GB ceiling's 48/24. Acceptance: the new rank is recorded with its VRAM cost; it was a constraint before, not a choice.
- [ ] Emit samples every epoch on the pinned set. Acceptance: one sheet per epoch, epoch-labelled, same prompts and seeds throughout the run.

## P4 — The user selects, then ship or say so

- [ ] Have the user pick the best epoch from the sheets. Acceptance: a chosen epoch recorded with their reasoning; the final epoch is not assumed to be the best.
- [ ] A/B the chosen epoch against the `e07` bar and review the images before accepting a verdict. Acceptance: gate + `iou_ref` + the user's read on south framing and east pose, recorded together.
- [ ] Run the anteater generalisation check on the candidate. Acceptance: its three views reviewed by eye — a species with no corpus analogue is the honest test.
- [ ] Ship it or state plainly that it lost. Acceptance: `completed.md` carries the verdict and its sheet; a loss is recorded as a result, not retried by reflex.
- [ ] Run `bin/rd docs-check` and close the stream. Acceptance: tree green, index row updated, `generate.py` left pointing at whatever actually ships.
