# Forks — beat e07

_A choice I resolved, with what was rejected and why. A fork is mine; a [blocker](blockers.md) is
the user's._

## F1 — The shape of the selection loop {#f1}
_2026-08-02 · resolved at plan time · **single-variable runs the user picks between, not
breed-from-the-winner**_

**Chosen.** Each round changes exactly ONE thing, the user picks the winner from images, and the
next round changes exactly one more thing starting from that winner's *settings*. Within a run,
samples every epoch let the user pick the best **epoch**.

**What the user proposed**, and why the mechanism doesn't hold: "run 10 generations, I select the
best, we then build another 10 off of that." Two readings, both problematic.

- *If the ten are epochs of one run* — picking the best and continuing from it is just training
  longer. Epoch 7 → 8 is what training already does; the selection adds no information.
- *If the ten are configs* — forking from the winner's WEIGHTS and training ten more compounds
  overfitting. Run-4 was already 15 epochs on 459 images (765 effective); continuing to 25+
  memorises.

Genetic search doesn't map onto gradient descent: there is no crossover between two LoRAs, and
"more training from the winner" is not mutation, it is the same descent going further. Selection
creates value only when the candidates differ along an axis someone chose — which is what this
fork keeps, at a tenth of the cost.

**The cost that decided it.** Run-4 took 4 h 39 m on the 2080 Ti; call it ~2.5 h on the 3090. Ten
runs is 20–25 GPU-hours per round and 30 sample sets for the user to review, three rounds is
60–75 hours — spent searching CONFIG, which has never been implicated, while the DATASET, which
correlates with all three failures, stays untested.

**Kept from the user's proposal, because it is right:** the user's eyes decide, samples come out
every epoch rather than at the end, and no automated score ships a model on its own.

## F2 — Which axis to attack first {#f2}
_2026-08-02 · resolved at plan time · **the P2 dataset rebuild, decomposed**_

**Chosen.** Test the three P2 changes — upscaler, scale normalisation, resolution — one at a time
against the `e07` baseline, before touching any training config.

**Why.** `e07` is the only model trained on **v1** and the only model that wins. Runs 3, 4 and 5
are the only models trained on **v2** and are the only models that lose. That is a perfect
correlation nobody has tested, because P2 shipped all three changes together and every run since
inherited all three.

**Why normalisation goes first.** Run-4's south failure was diagnosed as a learned white margin
from pinned `fill`, and run-5 refuted that — but only over sd 0.003 → 0.027, while `e07`'s data sat
at **0.148** ([I1](issues.md#i1)). The refutation stopped 5.4× short of the shipping condition, so
normalisation is the one P2 change with a half-tested hypothesis already attached to it.

**Rejected — train a bigger/longer model on the 3090 first.** It is the tempting use of new
hardware and it changes the variable that has never been shown to matter. If v2 is the problem, a
bigger model trained on v2 is a more expensive way to lose.

**Rejected — go straight back to v1 and ship that.** It would probably beat run-5, and it would
teach nothing: v1 also has the scale drift and soft outlines P2 was built to fix. The point is to
learn which of the three fixes cost more than it bought.

**Rejected — jump to the lineart LoRA instead.** Different product, still open on its own stream,
and it inherits the same dataset questions. Answering these first de-risks it.

## F3 — What "the user selects" is allowed to select {#f3}
_2026-08-02 · resolved at plan time · **the eye ratifies; `iou_ref` is the tiebreak**_

**Chosen.** Every A/B reports gate + `iou_ref` + signed aspect AND emits the visual sheet, and no
verdict is accepted before the images are looked at. Where they disagree, the images win and the
disagreement is recorded as a finding.

**Why not "the metrics decide".** Four times in this project the images overturned the numbers —
the anteater (metric rated the visually best output worst), run-4 south (passed 10/18 while broken),
run-4 east (scored a tie while the pose convention broke), and run-4 overall ("slightly worse"
aggregate hiding "better on east, broken on south").

**Why not "the eye alone".** `sprite-eval-trust` spent a whole stream making the ruler agree with
the eye and succeeded — `iou_ref` called both run-4 failures 6/6 where `d_aspect` scored a tie. A
statistic that now agrees is worth keeping as the cheap first pass and the record of record;
discarding it would throw away the one thing that stream bought.
