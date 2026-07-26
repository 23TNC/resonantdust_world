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
