# Visual consistency across the three views — plan

Items are checkboxes; tick in place (`[x]`), never move them. One action + acceptance each.
The LoRA is **frozen** at `rd_diremph_anima_r20_g07` ([checkpoints.md](checkpoints.md)) and the
control knobs are frozen at `cn 0.5 / cn-end 0.9 / strength 0.7`. **The only thing this stream
varies is the pipeline.** `cn` must not go below 0.5 ([README](README.md) future intent).

## P0 — A ruler before a change

- [x] Add a `--metrics` flag to `generate.py` printing per-direction luminance, saturation and coverage over opaque pixels. Acceptance: one line per direction on every run, so no sheet is ever judged by eye alone.
- [x] Emit the cross-direction luminance spread as a single number beside the sheet. Acceptance: the run prints `spread=NN.N` against the corpus ceiling 9.0.
- [x] Pin a consistency eval set — species, seeds, directions — in a file the harness reads. Acceptance: a file; two invocations produce the same cells, as `eval_set.json` does for the LoRA harness.
- [x] Record the baseline for r20g07 on the pinned set. Acceptance: a table in `issues.md`; today's single measurement (24.9 on wolf) is confirmed across species or corrected.

## P1 — The prompt form (cheapest lever, already evidenced)

- [ ] Replace `generate.py`'s prose `STYLE` with the LoRA's trained tag form, per direction. Acceptance: the emitted positive prompt for `--dir s` opens `rd_south, rd_south, rd_south, rd_style, rd_animal`.
- [ ] Keep the prose form reachable behind a flag for non-tag LoRAs. Acceptance: `--style-form prose|tags` selects; `e07` and any future prose-captioned model still work.
- [ ] Re-measure the spread with tags versus prose on the pinned set. Acceptance: two numbers; today's evidence says the cyan/peach artifacts vanish ([I1](issues.md#i1)) — confirm it moves the spread or say it does not.

## P2 — Anchor every view on every other view

- [ ] Put EAST through the same graph as south and north ([I6](issues.md#i6) — east is the outlier in 11/12 sets because it alone skips the IP-Adapter). Acceptance: the spread after east is self-anchored, against the 23.4 baseline.
- [ ] Replace `weight_type: "style transfer"` with a composition-carrying type in `graph_ip`. Acceptance: a sweep over the available types; the one that best holds markings is recorded with its sheet.
- [ ] Anchor south and north on EACH OTHER via `IPAdapterBatch`. DEPRIORITISED by [I6](issues.md#i6): they already agree at 7.7 mean against a 9.0 ceiling. Acceptance: it improves |s-n| or is dropped as unnecessary.
- [ ] Try `IPAdapterStyleComposition` with the hero as style and the template as composition. Acceptance: one sheet; this is the node that matches the actual division of labour, so it gets its own trial.
- [ ] Sweep `IP_WEIGHT` at the winning weight type. Acceptance: three values; the point where identity holds without the template being overpowered is recorded.

## P3 — One sampler pass for all three views

- [ ] Build a batch-3 graph: three latents, three control images, one KSampler. Acceptance: one call returns three images; peak VRAM recorded against 24576 MiB.
- [ ] Measure the spread from the batched pass against the sequential hero chain. Acceptance: both numbers on the same species and seed — this is the 24 GB capability the stream exists to test.
- [ ] Decide whether batching replaces the hero chain or supplements it. Acceptance: a fork entry naming the winner, with the loser's sheet kept.

## P4 — Lock value and palette

- [ ] Measure whether the drift survives every P1–P3 change. Acceptance: the spread after the best combination, against 24.9 baseline and the 9.0 ceiling.
- [ ] If drift survives, quantise all three views to one palette derived from the hero. Acceptance: a deterministic post-process; spread after quantisation, and a sheet proving the outline survived it.
- [ ] Check the absolute value gap separately from the spread. Acceptance: our 134–159 against the corpus 62–121 — a set that is internally consistent and uniformly too pale is only half fixed.
- [ ] Verify the layer-map decomposition still finds material regions after any palette change. Acceptance: `art maps --layers` on one sprite; flat-region count is not reduced.

## P5 — Close out

- [ ] Generate a full three-view set for a species with NO authored template analogue. Acceptance: the honest generalisation test, same as the anteater check in the predecessor.
- [ ] State the final spread and whether it clears the corpus ceiling. Acceptance: `completed.md` carries the number; missing the ceiling is a result, not a reason to retry by reflex.
- [ ] Run `bin/rd docs-check` and close the stream. Acceptance: tree green, index row updated.

## P6 — The edit-model track (F5/D1) — the design doc's first choice, finally tried

Runs beside P0–P4, not instead of them. Judged on the SAME ruler, which is architecture-independent.

- [ ] Download the Qwen-Image-Edit transformer onto the box, to `models/diffusion_models`. Acceptance: a weight file present; the VAE and Qwen text encoder are already staged so nothing else is needed.
- [ ] Generate the east hero, then ask the edit model for the south view of THAT wolf. Acceptance: one image; the question is identity preservation, not sprite quality.
- [ ] Run the pinned consistency set through the edit-model path. Acceptance: `results.json` under a `qwen` label, directly comparable to `baseline`.
- [ ] Compare edit-model spread against the best SDXL+IP-Adapter spread. Acceptance: two numbers on the same subjects and seeds, plus the sheet; the eye ratifies.
- [ ] Record which mechanism becomes the S/N default. Acceptance: a fork entry; if the edit model wins, say plainly that P2's anchoring work is superseded.

