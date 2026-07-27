# Sprite eval trust — plan

Items are checkboxes; tick in place (`[x]`), never move them. One action + acceptance each.
Ordering in [`README.md`](README.md): **fix the ruler first**, then diagnose, then retrain.

## P0 — Make the measurement see what the eye sees

- [x] Add `iou_ref` to `lora_eval`: IoU of the generated silhouette against the real corpus sprite for that species+direction. Acceptance: a run-4 framed-bust south scores lower `iou_ref` than the matching e07 full-body sprite.
- [x] Check `iou_ref` separates the east pose drift. Acceptance: run-4 east `iou_ref` is lower than e07 east on ≥4 of the 6 A/B species, or the result is recorded as a refutation.
- [x] Add signed aspect error `d_aspect_signed` alongside the unsigned one. Acceptance: e07 east reports mostly positive, run-4 east mostly negative, on the existing `.staging/ab4/` images.
- [x] Score every gate variant against `.staging/gate-cal/labels.csv` and report false-pos/false-neg. Acceptance: a table of ≥4 variants incl. the current gate, with error counts, in `completed.md`.
- [x] Adopt the lowest-error variant as the gate. Acceptance: `forks.md` records the choice, its error count vs the current 3, and what was rejected.

## P1 — Make the visual check automatic

- [x] Add a `contact_sheet()` helper to `lora_eval` that tiles labelled images into one PNG. Acceptance: called with 2 configs × 6 species, it writes a grid with readable column headers.
- [x] Emit a contact sheet from every `--pipeline` sweep and A/B run. Acceptance: a sweep writes `sheet.png` beside `sweep.csv` without any extra flag.
- [x] Emit a sheet from `generate.py --candidates`. Acceptance: a 4-candidate batch writes one sheet showing all candidates with their gate verdict in the label.

## P2 — Fix the frame artefact

- [x] Jitter the fill fraction per image in `prep_train.normalise()`. Acceptance: over a 60-image sample, longer-side fraction has mean ≈0.85 and sd between 0.02 and 0.05 (not the current 0.003).
- [x] Make the jitter deterministic per source file. Acceptance: prepping the same file twice yields byte-identical output; two different files get different fills.
- [x] Rebuild the 1024 dataset with jitter. Acceptance: 459 images, sd in range, outline sharpness still ≥85 (the P2 gain is not lost).

## P3 — Diagnose the pose drift before spending another 5 hours

- [x] Compare run-4 east sprites against their own training images for the same species. Acceptance: a sheet showing whether the corpus east sprites are lying or sitting, recorded in `issues.md`.
- [x] Test whether the drift is inference-side by generating run-4 east at 768 and 1024. Acceptance: signed aspect error at both resolutions, stated in `completed.md`.
- [x] Name the most likely cause from the evidence. Acceptance: `issues.md` states the cause, the evidence for it, and the candidates ruled out.

## P4 — Retrain with the fixes and prove it

- [x] Retrain on the jittered 1024 dataset, holding run-4's other settings. Acceptance: the run completes with per-epoch checkpoints under `output_quad3/`.
- [ ] A/B the result against `e07` and run-4 with the P0 gate. Acceptance: gate pass, `iou_ref` and signed aspect for all three, 6 species × e/s × 3 seeds, tabulated.
- [ ] Review the auto-emitted contact sheet before accepting any verdict. Acceptance: `completed.md` states what the images show and whether it agrees with the numbers.
- [ ] Choose the shipping checkpoint. Acceptance: `forks.md` names the winner and margin; if nothing beats `e07`, that is recorded and `e07` stays.
