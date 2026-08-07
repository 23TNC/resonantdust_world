# Style, not bestiary — plan

Items are checkboxes; tick in place (`[x]`), never move them. One action + acceptance each.
Config is **frozen** at the predecessor's settled values ([README](README.md)) — animagine, full
701 corpus, 768, `dim 64 / alpha 64`, `LR 5e-5`, batch 4, bf16, AdamW, single stage, stop at 12,
samples every generation on the pinned set. **The only variable this stream introduces is the
caption.**

## P0 — Measure the corpus before changing it

- [x] Count images per species per direction across all 200 folders. Acceptance: a table in `issues.md`; the claim "176 of 200 have exactly 3" is confirmed or corrected.
- [x] Count how many DISTINCT species tokens the captions currently carry. Acceptance: a number — it is how many concepts the current captioning asks one LoRA to learn.
- [x] Record what a caption looks like today, verbatim, for one image. Acceptance: the exact string in `issues.md`, so the diff to the style-only form is legible.

## P1 — Build the style-only caption variant

- [x] Write the style-only caption rule: convention tags + body plan + direction, NO species or family token. Acceptance: the rule is stated in `forks.md` with what it drops and why.
- [x] Generate the variant caption set beside the originals, leaving image files untouched. Acceptance: 701 new `.txt`, same stems, originals unmodified and still present.
- [x] Diff ten captions old-vs-new by eye. Acceptance: species and family tokens gone, convention and body-plan tokens intact, no empty captions.
- [x] Assert no caption is empty or duplicate-only. Acceptance: a script check — an all-identical caption set teaches nothing, the same defect shape as `rd_quadruped` on 459/459 (I12 there).

## P2 — Train it

- [x] Train run-16 on the style-only captions, config frozen, 12 generations. Acceptance: `TRAIN_EXIT_OK`, 12 checkpoints, 60 samples, loss curve recorded.
- [x] Verify at launch that it read the NEW captions, not the originals. Acceptance: the caption dir in the log matches the variant path — a silent fallback to the old set would invalidate the whole run.
- [x] Save all 12 generations under the naming scheme as `rd_styl_anima_r16_g<NN>`. Acceptance: 12 files in `models/loras`, provenance in `checkpoints.md`.

## P3 — Judge it against the species-captioned baseline

- [x] Build one sheet: run-16 vs run-10 at matched generations, same prompts and seeds. Acceptance: the user can compare in a single image.
- [ ] Have the user pick a generation, or state that none is usable. Acceptance: a choice recorded with their reasoning, judged on silhouette mass and value.
- [x] Run the anteater check on the chosen generation. Acceptance: three views by eye — a species with no corpus analogue is what a style-only LoRA should be BEST at, so this is the thesis's sharpest test.
- [x] Generate a species the corpus barely covers (cat, 3 images) and one it covers well. Acceptance: if the gap between them narrows versus run-10, the thesis holds; if not, say so.

## P4 — Through the real pipeline, with the knobs corrected

- [x] Re-run the ControlNet pipeline at stronger control than the default recipe. Acceptance: `--cn 0.5 --cn-end 0.9` versus the `dn=1 cn=0.2` that let the LoRA erase the template.
- [x] Sweep LoRA strength 0.5 / 0.7 / 0.85 on one direction. Acceptance: three sprites; the strength where the template survives is recorded.
- [x] State whether the pipeline produces usable art from this LoRA. Acceptance: a plain yes or no in `completed.md`, with the sheet.

## P5 — Close out

- [ ] Compare the chosen generation against `e07` on gate + `iou_ref` + the eye. Acceptance: the first head-to-head this project has actually run; numbers and images recorded together.
- [ ] Ship it or state plainly that it lost. Acceptance: `completed.md` carries the verdict; a loss is a result, not a reason to retry by reflex.
- [ ] Run `bin/rd docs-check` and close the stream. Acceptance: tree green, index row updated.

## P6 — Direction: emphasis, then isolation (F2)

Frozen across all three: animagine, 768, `dim 64 / alpha 64`, `LR 5e-5`, batch 4, bf16, AdamW,
single stage, 12 generations / **2112 steps**, run-16's pinned sample file unchanged.

- [ ] R20 — direction tag moved to caption head, repeated 3x, `keep_tokens=6`, full 701 corpus. Question: was I8 the cause? Acceptance: wolf-south at matched generations against r16 — the tail is a tail, or it is not.
- [ ] R17 — south-only corpus, 234 images at repeats 3. Question: does isolating south from north fix it? Acceptance: wolf-south silhouette at g08/g10 against r16g08.
- [ ] R18 — east-only corpus, 234 images at repeats 3. Question: is 234 above the data floor? Acceptance: east against r16g08-east, which is already known good — this is the control that makes R17 readable.
- [ ] Build one sheet: r16g08 / R20 / R17 / R18 on the same wolf prompts and six seeds. Acceptance: the user can compare in a single image.
- [ ] Record the winner and whether the split is needed at all. Acceptance: a line in `completed.md` naming which of emphasis-only, split, or neither fixed north/south.

