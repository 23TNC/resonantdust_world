# Can we measure a good sprite? — plan

Items are checkboxes; tick in place (`[x]`), never move them. One action + acceptance each.
Human labels under `.staging/eval_data/` are ground truth. **Every accuracy is leave-one-out** —
a threshold fitted and scored on the same points is how a useless metric looks useful.

## P0 — The labelled set and the agreement check

- [x] Build a tool that generates pipeline sprites into `new/` for hand sorting. Acceptance: `art eval-data`; never deletes, never re-offers a seed present in any bucket.
- [x] Score every labelled image against every metric we compute. Acceptance: `art eval-agreement`; per-direction table of good/bad means and best single-threshold accuracy.
- [x] Report accuracies leave-one-out, not fitted-in-place. Acceptance: both numbers exist and differ; the report states which it is showing.
- [x] State plainly whether `iou_ref` predicts the human. Acceptance: a number per direction against the majority-class baseline.

## P1 — Grow the set until a verdict is trustworthy

- [ ] Get south and north to at least 40 labelled images each. Acceptance: counts in `issues.md`; 19 and 13 cannot separate a real discriminator from a coincidence.
- [ ] Re-run the agreement check at the larger n. Acceptance: does `sat` survive on south, or was 95% an artifact of coloured junk in the rejects?
- [ ] Label a second species so metrics are not fitted to wolves. Acceptance: bear or fox at n≥20 per direction; a wolf-only ruler is not a ruler.

## P2 — Close the resolution question {#p2}

- [ ] Generate south at 768 — the resolution the LoRA was actually trained at. Acceptance: the same 3 seeds as the 512/1024 test, side by side.
- [ ] Decide the generation resolution on evidence. Acceptance: a fork naming 512, 768 or 1024 with the sheet; 1024 costs ~4x the compute of 512, so it must earn it.
- [ ] Re-measure the east six-species set at the chosen resolution. Acceptance: `iou_ref` against the 0.852 recorded at 512 — this tells us whether 512 was hurting east too.
- [ ] Wire the chosen resolution into `generate.py` as the default. Acceptance: the shipping path uses it without a flag; the old value recorded for rollback.

## P3 — What to do about a ruler that does not work

- [ ] State whether ANY single metric is usable for generation selection. Acceptance: a plain yes/no per direction in `completed.md`, with n.
- [ ] Try a simple combination rather than a single threshold. Acceptance: two or three metrics in a rule, scored leave-one-out; it beats the best single metric or it does not.
- [ ] If no metric works, make human sorting cheap instead. Acceptance: a sorting sheet or CLI that reduces a labelling pass to minutes.

## P4 — Close out

- [ ] Record what generation selection should do from now on. Acceptance: `completed.md` names the procedure, whether automated or not.
- [ ] Run `bin/rd docs-check` and close the stream. Acceptance: tree green, index row updated.
