# A north/south evaluation worth training against — plan

Items are checkboxes; tick in place (`[x]`), never move them. One action + acceptance each.
Everything is scored on **pipeline output**, never kohya samples. **Every accuracy is leave-one-out.**
Every table carries the majority-class baseline; at or below it is reported as failing.

## P0 — Labels worth fitting to

- [x] Settle the generation resolution before collecting any new labels. Acceptance: 768 (the LoRA's own) against 1024 and 512 on three fixed seeds; a fork names the winner — south's existing labels were collected at 512 and describe that, not the direction.
- [x] Add a reason code to the labelling flow. Acceptance: a reject records one of `background`/`lineart`/`anatomy`/`pose`/`colour`/`other`; a bare good/bad label cannot distinguish a fixable defect from a borderline call.
- [x] Add a `borderline` label distinct from `bad`. Acceptance: the user marked 2 of 3 east rejects "would accept if pressed"; scoring those as hard failures penalises a judge for agreeing with them.
- [ ] Grow south to n≥40 labelled at the settled resolution. Acceptance: a count in `issues.md`; 19 cannot separate a real discriminator from a coincidence.
- [ ] Grow north to n≥40. Acceptance: a count; north is the weakest cell at n=13 and every metric there is currently noise.
- [ ] Label a second species across both directions. Acceptance: bear or fox at n≥20 per direction — a judge fitted only to wolves is not a judge.

## P1 — The vision QA gate the design doc specifies {#p1}

- [x] Write the gate: one image in, a structured verdict out. Acceptance: single creature / correct facing / clean silhouette / on-style, each with a reason, on the doc's own four questions.
- [x] Score the gate against the labelled set. Acceptance: agreement per direction against the majority baseline — the same bar every metric has been held to.
- [ ] Report where the gate and the user disagree, with the images. Acceptance: a sheet; a judge that fails on the borderline cases is usable, one that fails on clear rejects is not.
- [x] Measure cost and latency per image. Acceptance: seconds and tokens per sprite; a gate too slow for 12 generations × N species is not a selection procedure.

## P2 — Cheap deterministic checks, kept as a pre-filter

- [ ] Add the unkeyed-background check to the gate. Acceptance: corner alpha; it flags the one image in 63 the user rejected for a white background and nothing else.
- [ ] Audit which existing gate metrics ever fire on real rejects. Acceptance: a table — `blobs` was constant at 1 across all 63 labelled images and caught nothing.
- [ ] State which deterministic checks earn their place. Acceptance: a list in `completed.md`; the rest are removed rather than left as decoration.

## P3 — Interior metrics, properly powered {#p3}

- [ ] Re-test left-right asymmetry on south at n≥40. Acceptance: does 3-wrong-of-19 survive; the mechanism I proposed is falsified, so this needs power rather than a story.
- [ ] Test interior metrics that target line art specifically. Acceptance: something that separates the user's `lineart` reason code from the rest, or a plain statement that none does.
- [ ] Compare interior REGION LAYOUT against the corpus sprite, not just the silhouette. Acceptance: quantise both and compare region maps; this is the untried half of `iou_ref`.

## P4 — A selection procedure for the next training run {#p4}

- [ ] Combine whatever survived into one ranking rule. Acceptance: scored leave-one-out against the full labelled set; it beats the best single component or it is dropped.
- [ ] Run the rule across run-20's 12 generations on pipeline output. Acceptance: a ranking with the sheet — the first generation-to-generation comparison this project has made on output that resembles what ships.
- [ ] Check the rule's pick against the generation the user chose by eye. Acceptance: g07 either ranks near the top or it does not; a disagreement is a finding, not a bug to hide.
- [ ] Write the procedure down as the standing method. Acceptance: `completed.md` names it, including the human step if one survives.

## P5 — Close out

- [ ] State plainly whether north/south selection can be automated. Acceptance: a yes/no per direction with n; "no" is a result and means the human loop gets made fast instead.
- [ ] Run `bin/rd docs-check` and close the stream. Acceptance: tree green, index row updated.
