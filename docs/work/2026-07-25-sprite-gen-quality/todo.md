# Sprite-gen quality — plan

Items are checkboxes; tick in place (`[x]`), never move them. One action + acceptance each.
Ordering argument in [`README.md`](README.md): **P0 fixes the measuring instrument first**, because
every later comparison is scored by it.

## P0 — Fix the ruler (the gate has both error types)

- [x] Assemble a hand-labelled calibration set from this session's sprites. Acceptance: `.staging/gate-cal/labels.csv` has ≥30 rows of `path,verdict` where verdict is `good`/`bad`, covering the anteater and oryx cases.
- [x] Add `iou_control` to `lora_eval.measure()` — IoU of the output silhouette against the control image it was given. Acceptance: the blobby anteater south scores lower `iou_control` than its own good east.
- [x] Replace the flat `solidity` check with one that tolerates horns/antlers. Acceptance: the oryx candidate at `solidity=0.349` passes; a genuinely fragmented sprite still fails.
- [x] Re-tune the gate thresholds against the calibration set. Acceptance: false-positive and false-negative counts are both printed, and both are lower than the current gate's on the same set.
- [x] Add `--seeds N` to `tf_baseline.py` so every cell is a rate, not a single roll. Acceptance: `--seeds 3` writes 3 rows per species+direction and prints a per-cell pass rate.

## P1 — Choose the control silhouette by measurement, not by a hand table

- [x] Add a shape-similarity search over the 396 bank silhouettes. Acceptance: a function returns the top-k nearest bank entries for a target profile, and `Bear` is nearer `Bear` than any non-bear.
- [x] Add `--control auto` selecting the nearest bank silhouette per direction. Acceptance: `--control auto` generates a full e/s/n set with no family argument.
- [x] Re-run the anteater with `--control auto`. Acceptance: south/north `iou_control` and gate verdict both beat the `family:pachyderm` run, recorded in `completed.md`.
- [x] Decide the fate of `FAMILY_REP` once `auto` is measured. Acceptance: `forks.md` records keep/demote/retire with the numbers that decided it.

## P2 — Fix the training data (the biggest single lever)

- [ ] Replace LANCZOS in `prep_train.py` with an edge-preserving upscale. Acceptance: a 64px `Cat` at 768 shows higher mean outline gradient magnitude than the LANCZOS version; both images saved for comparison.
- [ ] Normalise subject scale so every sprite fills a fixed fraction of the frame. Acceptance: alpha-bbox area / frame area is 0.80 ±0.03 for every prepped image.
- [ ] Rebuild the training set and report the before/after distribution. Acceptance: `fill` standard deviation across the corpus drops below 0.02, stated in `completed.md`.
- [ ] Spot-check 6 rebuilt sprites against their sources for artefacts. Acceptance: 6 side-by-side pairs saved; no halo, clipping, or aspect distortion found, or the defect is logged in `issues.md`.

## P3 — Retrain and prove it is better

- [ ] Retrain the quadruped LoRA on the rebuilt data at the run-3 recipe. Acceptance: the run completes with per-epoch checkpoints under `output_quad2/`.
- [ ] A/B the new LoRA against `rd_quadruped_e07` multi-seed. Acceptance: pass-rate and mean `d_aspect` for both over 10 species × e/s/n × 3 seeds, tabulated in `completed.md`.
- [ ] Choose the shipping checkpoint. Acceptance: `forks.md` names the winner and the margin; if the new LoRA loses, that is recorded and `e07` stays.

## P4 — Run one full flywheel cycle

- [ ] Mass-generate a curation pile across ≥15 species with `--candidates`. Acceptance: ≥200 sprites written with per-batch `scores.csv`.
- [ ] Present the gate survivors to the user as contact sheets for hand-picking. Acceptance: sheets saved under `.staging/curate/`, and the user's keep/reject list is recorded.
- [ ] Fold the hand-picked sprites into the training set as extra data. Acceptance: the rebuilt set reports how many images came from generation vs the original corpus.
- [ ] Retrain on corpus + curated and A/B it. Acceptance: same multi-seed comparison as P3, against both `e07` and the P3 winner.

## P5 — Tighten cross-direction coherence

- [ ] Sweep IP-adapter weight against the coherence metric. Acceptance: a table of weight → mean cross-direction colour distance over ≥3 species.
- [ ] Implement the best-scoring setting as the default. Acceptance: mean cross-direction distance drops below 0.10 (from 0.169) without a gate pass-rate regression.
