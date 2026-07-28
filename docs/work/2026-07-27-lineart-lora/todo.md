# Line-art LoRA — plan

Items are checkboxes; tick in place (`[x]`), never move them. One action + acceptance each.
Rationale in [`README.md`](README.md).

## P0 — Decolour the corpus

- [x] Write `decolour_corpus.py` reconstructing each sprite as `residual + Σ(layers×TINT)` with the source alpha re-applied. Acceptance: a smoke run yields subject luminance 0–200 against a 255 plate.
- [x] Choose the tint by measuring subject/plate separation. Acceptance: a table of candidate tints with the brightest subject pixel and its gap to 255; the choice is justified by it.
- [x] Build the full decoloured corpus. Acceptance: ≥695 of 701 sprites written, and any sprite that fails is named rather than silently dropped.

## P1 — Build the trainable set

- [ ] Filter the grey corpus to quadrupeds and prep at 1024 via `build_quad --src`. Acceptance: 459 images / 459 captions across the three repeat-weighted folders, all 1024².
- [ ] Verify the prep did not reintroduce colour. Acceptance: mean per-pixel saturation over 60 sampled images is below 0.02.
- [ ] Confirm scale normalisation still holds on grey input. Acceptance: longer-side fraction 0.85 ±0.05 over a 60-image sample.
- [ ] Spot-check 6 prepped images against their colour originals. Acceptance: a sheet saved; markings and outlines present in all 6, or the defect is logged.

## P2 — Train

- [ ] Train at the run-5 recipe on the grey set, changing only the data. Acceptance: the run completes with per-epoch checkpoints under `output_line/`.
- [ ] Generate the 5 sample prompts from the best epoch. Acceptance: a contact sheet showing e/s/n for at least 3 species.

## P3 — Does it generalise past the corpus?

- [ ] Ask the trained LoRA for an anteater line drawing. Acceptance: 3 seeds saved; `completed.md` states whether the snout/tail form appears or it returns a corpus body plan.
- [ ] Compare its line art against the 132-entry bank for a species the bank covers. Acceptance: `iou_ref` for both, on ≥4 species, tabulated.
- [ ] Decide whether the LoRA replaces or supplements bank lookup. Acceptance: `forks.md` records the choice with the numbers; "neither" is a valid outcome.
