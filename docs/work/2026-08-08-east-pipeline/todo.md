# The east pipeline — plan

Items are checkboxes; tick in place (`[x]`), never move them. One action + acceptance each.
**East only.** LoRA frozen at `rd_diremph_anima_r20_g07`. Trained species are the calibration set
([P1](#p1)–[P3](#p3)); untrained species are the product ([P4](#p4)) and come after the method is
settled. A labelled run owns the code it started with ([predecessor D2](../2026-08-08-direction-consistency/deviations.md)).

## P0 — Ground truth and a ruler that can see a wrong animal

- [x] Pick 6 trained species spanning body plan and colour, all with a corpus east sprite. Acceptance: a list in `eval-east.json`; each has an `e.png` in the silhouette bank and a real sprite for `iou_ref`.
- [x] Build an east harness reporting `iou_ref`, signed aspect, interior lum/sat, and the gate. Acceptance: one row per species per method; `iou_ref` is against the REAL sprite, never against the control.
- [x] Score the REAL corpus sprites through the harness as a self-check. Acceptance: `iou_ref` = 1.0 for every species — if it is not, the reference lookup is wrong and every later number is meaningless.
- [x] Record the S0 incumbent baseline: wolf template on all 6 species. Acceptance: a table; this is the number the bear-as-wolf defect actually scores, and the floor every method must beat.

## P1 — The stage ladder, measured {#p1}

- [x] S1 — generate each species against its OWN corpus silhouette (`--control corpus:<Species>`). Acceptance: `iou_ref` against S0; the cheapest candidate fix gets measured before any pipeline is built.
- [x] S3 — run `--control auto` (probe → nearest bank body plan). Acceptance: `iou_ref`, plus which species it picked for each — a wrong pick is the interesting failure, not the score.
- [x] S4 — add a low-denoise refine pass over the S1 winner. Acceptance: `iou_ref` and the eye on outline cleanliness; it earns its stage or it is dropped.
- [x] S5 — generate N candidates per species and keep the best by `iou_ref`. Acceptance: best-of-N against best-of-1 at the same total GPU cost, so the comparison is fair.

## P2 — Silhouette then texture, the user's proposal, done properly {#p2}

- [x] Define what "the silhouette stage" emits — filled mask, lineart, or sprite-on-white. Acceptance: a fork entry; `edge_map()` stays the single place edges are derived, per `silhouette_bank`'s own rule.
- [x] Build stage 1: produce a species-correct east silhouette without a hand-authored template. Acceptance: a silhouette whose `iou_ref` beats the wolf template's, judged as a silhouette alone.
- [x] Build stage 2: paint the interior with the silhouette as control. Acceptance: an end-to-end sprite; `iou_ref` and interior metrics against S1.
- [x] Split the knobs per stage. Acceptance: `cn`/`dn`/LoRA strength chosen separately for shape and for paint, with the measured reason each differs.
- [x] State whether two stages beat one. Acceptance: a plain yes or no with the table; more stages cost wall-clock and failure modes, so a tie means one stage wins.

## P3 — Choose the method {#p3}

- [x] Build one sheet: all methods × 6 species, east, same seeds. Acceptance: the user can compare in a single image.
- [ ] Have the user pick the method, or state none is good enough. Acceptance: a choice recorded with their reasoning.
- [ ] Wire the winner into `bin/art generate` as the documented default for east. Acceptance: the default path produces it without flags; the old behaviour stays reachable.

## P4 — Animals with NO training data {#p4}

- [x] Choose 4 untrained species, at least one with no close corpus body plan. Acceptance: named with the nearest bank species and how far off it is — the anteater precedent.
- [x] Run the P3 winner unchanged on them. Acceptance: sprites plus the honest read; `iou_ref` is undefined here, so the eye and the convention are the judges.
- [x] Identify what breaks that trained species did not show. Acceptance: a list in `issues.md` — the user expects extra or different steps, and naming them is the deliverable.
- [x] Propose the extra stages untrained species need. Acceptance: a fork entry with the reasoning; building them is the next stream if it is more than a knob change.

## P5 — Close out

- [ ] State the method, its `iou_ref`, and where it fails. Acceptance: `completed.md` carries all three; a partial answer is recorded as partial.
- [ ] Name what the other directions will need. Acceptance: a paragraph the follow-up stream can start from, since south and north were excluded by instruction, not by evidence.
- [ ] Run `bin/rd docs-check` and close the stream. Acceptance: tree green, index row updated.
