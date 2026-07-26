# Completed — verification log

_Append-only. Dated entries: what landed and **how it was checked**. Item text lives in
[`todo.md`](todo.md) with its box ticked._

_(nothing landed yet — the stream opened 2026-07-25 with the plan only)_

## 2026-07-26 — P0 complete (the ruler is calibrated, and two assumptions were refuted)

**Calibration set** — `.staging/gate-cal/labels.csv`, **67 sprites** (58 good / 9 bad), each row
carrying a `why`. Labels are this session's verdicts: the user's explicit calls where given ("the
row 2+ bears are perfect", "row 1 wolf south is pretty good", the anteater "doesn't quite look
right"), otherwise a visual judgement made against the montages. **Known weakness: these are not
blind labels**, and the good/bad split is imbalanced 58/9 because most generations genuinely are
fine. Deliberately ambiguous sprites (wombat, meerkat, the hornless oryx south) were **excluded**
rather than forced into a bucket. The tf-baseline sprites were re-examined visually before labelling
— they had only ever been seen as numbers — which confirmed all five template-free south failures
render a *side view* where a front view was asked for.

**`iou_control` and `hull_solidity` implemented, then refuted as gate checks** —
[I5](issues.md#i5), [I6](issues.md#i6). The anteater's bad views score *higher* `iou_control` (0.789
/ 0.732) than its good east (0.703), because IoU asks "did it follow the control?" and the control
was the wrong shape to begin with. `hull_solidity` separates good from bad at only 0.30 sd. Both are
kept as reported metrics — they measure real things — but neither is gated.

**Gate re-tuned on evidence ([F3](forks.md#f3)): errors 4 → 3.** `solidity` removed (0.19 sd of
signal, and it caused the oryx false negative — verified now passing). `d_fill` tried and rejected:
despite the second-best marginal separation (0.83 sd) it doubles-to-triples total error as a hard
gate. `blobs` retained as a zero-cost regression alarm.

**The residual 2 false positives are structural, not a tuning failure ([I7](issues.md#i7)):** for an
*unseen* species the gate's reference is a proxy from another species, so the anteater was judged
against an Elephant and its blob legitimately resembles elephant proportions. Geometry gating is
sound for corpus species and weak for exactly the unseen case template-free generation exists to
serve. Recorded rather than tuned away.

### Two caveats on the P0 boxes

**The `iou_control` item's acceptance FAILED, and the box is ticked anyway.** Its criterion was
"the blobby anteater south scores lower `iou_control` than its own good east" — the measurement came
back the opposite way (0.789 vs 0.703). The *work* is done and the metric ships as a reported value;
what is retired is the plan's **premise** that it would catch this case. That is a plan error, logged
as [I5](issues.md#i5). Ticking the box without this note would have misrepresented a refutation as a
success.

**Removing `solidity` has a real cost, accepted knowingly.** The item asked to *replace* it with a
horn-tolerant variant; the calibration data said the honest move was to **delete** it, since both it
(0.19 sd) and the hull-based replacement (0.30 sd) carry almost no signal. The consequence: a
single-component but genuinely *wispy* sprite is no longer caught — `blobs` only catches a shape
that fragments into separate pieces. No such sprite exists in the 67-sprite calibration set, so the
risk is untested rather than disproven. If wispy output shows up later, the fix is a targeted check,
not restoring a threshold that rejected good art.

## 2026-07-26 — P0 item 5: multi-seed baselines

`tf_baseline.py --seeds N` now rolls each species+direction N times, records the seed and the gate
verdict per row, and prints a **per-cell pass rate**. Verified with `--mode none --seeds 3`: **90
rows** (30 cells × 3), overall gate pass rate **74/90 = 82%**.

The aggregate barely moved from the single-seed run (83%), but the per-cell view is new information
the old baseline could not express — it separates *consistent* failure from *stochastic* failure:

| pattern | cells |
|---|---|
| consistently fails | **cat south 0/3** |
| mostly fails | bear south 1/3, pig south 1/3 |
| coin-flip | fox south 2/3, horse south 2/3, zebra south 2/3, several norths 2/3 |
| always passes | every east except cat (2/3); wolf, tiger all three |

This matters for where effort goes: a 0/3 cell is a capability gap (no amount of sampling fixes it),
while a 2/3 cell is exactly what `--candidates` already solves for free. The single-seed baseline
reported both as the same thing.

## 2026-07-26 — P1 complete (auto built and measured; the plan's premise refuted)

**Shape-similarity search** added to `silhouette_bank.py`: 32×32 occupancy IoU blended 0.7/0.3 with
aspect agreement, cached per direction. Verified — `Bear/e`'s nearest neighbours are Bear (1.000),
Capybara (0.899), `AEXP_BlackBear` (0.887).

**`--control auto`** is a two-pass resolve: generate one template-free **probe** (east, the only
direction that passed 10/10 in both modes), match it against the bank, then use that species for all
three directions. One species for the whole set — not a per-direction match — costs one extra
generation instead of three and keeps e/s/n coherent.

**A coherence bug found and fixed while testing:** the gate was still scoring against a hand-passed
`--ref`, i.e. measuring the output against a species whose silhouette was never used. It rejected a
perfectly good auto east purely for not resembling the Elephant it was never shaped by. `auto` now
sets the gate reference to its own pick unless `--ref` is explicit (and warns when they disagree).

**The acceptance criterion was NOT met, and the box is ticked with that recorded.** `auto` was
supposed to beat `family:pachyderm` on the anteater. It was **worse** — Gorilla removed the snout
entirely ([I9](issues.md#i9)). It also picked a **hedgehog for a wolf** ([I8](issues.md#i8)).

**The salvage is the part nobody planned: knowing when to decline.** Calibrated from real data —
species with a corpus twin score 0.705–0.967, the anteater 0.532 — `AUTO_MIN_MATCH = 0.65` routes
the anteater to **no control**, which is the visually best of the three modes (recognisable snout,
tail and shoulder marking in all three views, versus cape-blobs from the Elephant and snoutless
blobs from the Gorilla). Verified: `auto DECLINED (best match Gorilla 0.53 < 0.65)`.

**[F4](forks.md#f4): `FAMILY_REP` is KEPT.** It encodes semantics the measurement cannot recover.
`auto` ships as an addition whose real value is the decline path.
