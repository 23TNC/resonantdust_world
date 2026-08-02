# Beat e07 — plan

Items are checkboxes; tick in place (`[x]`), never move them. One action + acceptance each.
Ordering: **make the box able to train at all**, then **decompose P2 one variable at a time**
(the only axis that correlates with every failure), then spend the 3090's new headroom, then close.

## P0 — Make the rig trainable and the baseline reproducible

- [ ] Bring ComfyUI up on the 3090 and confirm torch sees the card. Acceptance: `/system_stats` answers on `:8188` and reports the 3090 with 24 GB.
- [ ] Decide whether ComfyUI should autostart, and record the choice. Acceptance: a line in `forks.md`; today it is absent from the unraid autostart list and its restart policy is `no`.
- [ ] Make `prep_train` FAIL LOUDLY when ComfyUI is unreachable instead of falling back to LANCZOS. Acceptance: with the box down, a rebuild exits non-zero rather than silently producing a different dataset ([I2](issues.md#i2)).
- [ ] Re-run the frozen A/B harness on `e07` alone to confirm the baseline still reproduces. Acceptance: 26/36 gate and `iou_ref` 0.753 on the same 6 species × e/s × 3 seeds, or the drift is recorded.
- [ ] Pin the sample set: fixed prompts, fixed seeds, fixed species, written to a file every run reads. Acceptance: two invocations on the same model produce byte-identical sample sheets.

## P1 — Decompose the P2 rebuild (the untested common factor)

- [ ] Rebuild the dataset at v1 settings EXCEPT the upscaler — ESRGAN, unnormalised. Acceptance: fill sd back near v1's 0.148 with outline sharpness near v2's 90.1, both measured over a 120-image sample.
- [ ] Train run-6 on that set, config identical to run-5. Acceptance: the ONLY delta versus run-5 is scale normalisation, stated in `completed.md` before the run starts.
- [ ] A/B run-6 against `e07` and review the image sheet before accepting any verdict. Acceptance: gate + `iou_ref` + the user's read on south framing and east pose, recorded together.
- [ ] If normalisation is exonerated, rebuild at v1 resolution (768) holding the winner's other settings. Acceptance: the delta versus the previous run is resolution alone.
- [ ] Train run-7 on that set and A/B it the same way. Acceptance: south framing and east pose compared at matched epochs against both `e07` and run-6.
- [ ] If resolution is exonerated too, rebuild with LANCZOS to close the last P2 variable. Acceptance: the delta is the upscaler alone; this is the last of the three.
- [ ] Record which P2 variable (if any) accounts for the south-framing regression. Acceptance: a named variable with its A/B evidence in `completed.md`, or an explicit "none of the three" finding.

## P2 — Per-epoch selection, the user's loop

- [ ] Emit samples every epoch on the pinned set for whichever run P1 leaves standing. Acceptance: 15 sample sheets from one run, same prompts and seeds, epoch-labelled.
- [ ] Have the user pick the best EPOCH from the sheets, not the best run. Acceptance: a chosen epoch recorded with the user's reasoning in `completed.md`.
- [ ] A/B the chosen epoch against `e07` and the run's final epoch. Acceptance: three-way gate + `iou_ref`, confirming or refuting that mid-run beats end-of-run as `e07`'s own provenance suggests.

## P3 — Spend the 3090's headroom, one turn at a time

- [ ] Measure peak VRAM for the standing config on the 3090. Acceptance: a MiB figure against 24576, so the batch/dim decisions below are sized on data not guesswork.
- [ ] Collapse grad-accum 4 into a real batch and retrain. Acceptance: the ONLY delta is effective batching; wall-clock and A/B both recorded.
- [ ] Switch fp16 to bf16 and retrain. Acceptance: the ONLY delta is precision; loss curve compared for the scaling instability bf16 is meant to remove.
- [ ] Raise `dim`/`alpha` past the 2080 Ti's ceiling and retrain. Acceptance: the ONLY delta is rank; A/B'd against the P3 winner so far.

## P4 — Close out

- [ ] State whether anything beat `e07`, and ship it or say plainly that nothing did. Acceptance: `completed.md` carries the verdict with its A/B sheet; a loss is recorded as a result, not retried by reflex.
- [ ] Run the anteater generalisation check on whatever ships. Acceptance: the anteater's three views reviewed by eye; a species with no corpus analogue is the honest test.
- [ ] Run `bin/rd docs-check` and close the stream. Acceptance: tree green, index row updated to `done`.
