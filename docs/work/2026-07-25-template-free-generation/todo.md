# Template-free generation — plan

Items are checkboxes; tick in place (`[x]`), never move them. One action + acceptance each.
Design stance in [`README.md`](README.md); open decisions in [`forks.md`](forks.md).

## P0 — Baseline: measure how good template-free already is

Nothing here changes the pipeline. It establishes the number every later phase is judged against,
because "template-free is worse" is currently an assumption, not a measurement.

- [x] Add `--control none` to `lora_eval.py --pipeline` so a sweep can run txt2img+LoRA with no template. Acceptance: the flag runs a species without reading `textures/<kind>/template.*`.
- [x] Score 10 species × e/s/n template-free, writing per-candidate metrics. Acceptance: `.staging/tf-baseline/scores.csv` has 30 rows with blobs/bg/fill/aspect/solidity.
- [x] Score the same 10 species × e/s/n templated (wolf template, `--control template`). Acceptance: a second CSV; both valid-rates stated in `issues.md`.
- [x] Set the geometry pass-gate from those two runs. Acceptance: a threshold that fails the known wolf-east bust and bear-east blob, recorded in `forks.md`.

## P1 — Retire the hard template requirement

- [x] Add `--control {template,corpus,family,none}` to `generate.py`, defaulting to `template`. Acceptance: `--help` lists it; every existing invocation behaves identically.
- [x] Make `load_template()` return `None` instead of `SystemExit` when the mode does not need art. Acceptance: a kind with no template file generates under `--control none`.
- [x] Skip the ControlNet nodes (30/31/32) when there is no control image. Acceptance: the submitted graph omits those keys; a run succeeds and the log states "no control".
- [x] Use `EmptyLatentImage` when there is no control image, and log that `--dn` is inert. Acceptance: `--control none --dn 0.5` warns and still produces a full-strength sprite.

## P2 — Corpus-derived control (the silhouette bank)

- [ ] Write `bin/lib/silhouette_bank.py` extracting a control silhouette per species+dir from `.staging/quad-lora-train`. Acceptance: `.staging/silhouette-bank/<Species>/<dir>.png` exists for all 132 species.
- [ ] Emit the bank's edge maps with the same `edge_map()` used by the pipeline. Acceptance: a bank edge map is byte-identical to `edge_map()` run on the same source sprite.
- [ ] Add a family → representative-species table covering the corpus families. Acceptance: every family in `build_quad.py` resolves to one representative; an unknown family errors listing valid names.
- [ ] Wire `--control corpus:<Species>` and `--control family:<f>` to source from the bank. Acceptance: `--control family:bear` on a bear prompt yields a bear-shaped sprite, not a wolf-shaped one.
- [ ] Re-run the P0 bear failure with `--control family:bear`. Acceptance: the bear-east blob and bear-face-in-torso are gone; scores beat the P0 templated run, recorded in `completed.md`.

## P3 — Generate-many + auto-screen (make failure cheap)

- [ ] Add `--candidates N` to `generate.py` producing N seeds per direction. Acceptance: one invocation writes N×3 sprites into per-seed variant leaves.
- [ ] Score every candidate with `lora_eval.measure` at write time. Acceptance: a `scores.csv` beside the batch with one row per generated sprite.
- [ ] Route candidates failing the P0 gate to a `_rejected/` leaf. Acceptance: a deliberately bad run leaves the good leaves clean and the rejects quarantined.
- [ ] Print the best seed per species+direction. Acceptance: the summary names one winning seed per direction with its score.

## P4 — e/s/n coherence without a template set

The east hero + IP-adapter anchor already exist; what is unproven is whether they hold when east
itself was generated rather than drawn.

- [ ] Measure colour/marking consistency of s/n against a template-free east. Acceptance: a histogram distance per direction pair, recorded in `completed.md` with a pass/fail call.
- [ ] Resolve where s/n control comes from, given east's silhouette cannot drive them. Acceptance: `forks.md` F3 records the choice and the rejected options.
- [ ] Implement the F3 choice. Acceptance: a species generates a coherent e/s/n set with no hand-authored art at any step.

## P5 — Validate on species absent from the training corpus

- [ ] Generate 5 species not present in `.staging/quad-lora-train`. Acceptance: 5 × e/s/n at the chosen settings, scored, valid-rate in `completed.md`.
- [ ] Record which body plans the method cannot serve. Acceptance: `issues.md` names the failing plans and what each would need (new family silhouette, or a new LoRA).
