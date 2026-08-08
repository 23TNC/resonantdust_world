# Can we measure a good sprite? — 2026-08-08

_Component: [`dev/art`](../../components/dev/) · `bin/lib/{eval_data,eval_agreement,lora_eval,generate}.py`.
Plan in [`todo.md`](todo.md); decisions in [`forks.md`](forks.md); findings in [`issues.md`](issues.md).
Opened after [`2026-08-08-east-pipeline`](../2026-08-08-east-pipeline/README.md) found that the
question underneath it was unanswered._

## The thing that started this

The user asked whether the images kohya emits during training are used **in** training. They are not
— dataset and latent cache stay at 701/701/701 all run, and the samples land in a different tree.
But they are the whole of **evaluation**: every LoRA generation this project has ever picked was
picked by looking at them.

And they are generated **without ControlNet**. Six east wolves from `r20g07` with no control:

| | what came out |
|---|---|
| 2 of 6 | not an animal at all — abstract colour blocks, grey stripes |
| 1 of 6 | a **sprite sheet**: a grid of small wolf heads |
| 1 of 6 | a cropped bust filling the frame |
| 2 of 6 | a standing wolf on a coloured background |

`mean iou_ref 0.505 · mean aspect 1.05` against the real wolf's 2.08. **Zero** in the convention.

So generation selection has been made on output that does not resemble what the pipeline ships. The
same instrument produced the six-seed sweeps that made `r16g08` look hopeless the same day it
rendered a clean three-view wolf through the real pipeline
([east-pipeline I5](../2026-08-08-east-pipeline/issues.md#i5)) — that was not a one-off mistake of
mine, it is the project's standard evaluation.

## What this stream is for

**Build a hand-labelled set of pipeline-generated sprites, then find out whether anything we compute
agrees with the human.** Not "is this sprite good" — "can a number stand in for your eye". That is
the question that decides whether generation selection can ever be automated, and it had never been
asked.

## The measurement, and it is not comfortable

The user sorted 63 pipeline-generated wolf sprites into `good-{e,s,n}` and `bad-{e,s,n}`. Every
metric we compute was scored against those labels, with a **leave-one-out** check — because a
threshold fitted and scored on the same points flatters itself.

```
                 n    baseline   iou_ref   d_aspect   best metric
  EAST          31       84%        84%       90%     d_aspect 90%
  SOUTH         19       74%        63%       63%     sat 95% (suspect)
  NORTH         13       62%        38%       46%     fill 77%
```

**`iou_ref` — the metric the east-pipeline stream ranked every method with — does not predict human
judgement in any direction.** It equals the majority-class baseline on east, and on north at 38% it
is actively *anti*-correlated: it prefers the sprites the user rejected.

The reason is mechanical and was predicted, just not measured:
[east-pipeline F4](../2026-08-08-east-pipeline/forks.md#f4) said "a correctly shaped bear painted in
the wrong colours scores perfectly". South's failures have the *right silhouette* — the control
guarantees it — and a broken face. `iou_ref` only sees silhouette, so it ranks a faceless sprite
above a good one whose outline wobbles.

**This does not retract the east-pipeline results.** `iou_ref` compared *methods in aggregate*
across 18 cells, where S0 0.731 vs S1 0.852 is a large gap driven by silhouettes that were genuinely
wrong. What it cannot do is pick a good sprite from a bad one *within* a method — which is exactly
what generation selection needs.

## The direction asymmetry, from the user's own labels

```
east    good 26 / bad  5     84%
north   good  8 / bad  5     62%
south   good  5 / bad 14     26%
```

South is the outlier, and the cause is resolution, measured
([I4](issues.md#i4)): the control image *is* the i2i latent, so the control's size sets the
generation size. The bank is **512**. The east subject spans 510 of those 512 pixels; the **south
subject spans 202** — a narrow column that leaves the frontal face about 100px. Generating at
**1024** produces real wolf faces on the same seeds that produced splayed white masks at 512.

**The LoRA was trained at 768**, not 1024 and not 512 — verified from `train_run20.sh` and the
latent cache stamps (`_0768x0768_sdxl.npz`). So all three numbers differ and **768 is untested**;
that gap is [P2](todo.md) and it is the first thing to close, not a detail.

## Design stance

- **The human labels are ground truth here.** The corpus is ground truth for *fidelity*; for
  "is this sprite acceptable" only the user's sorting is.
- **Held-out or it did not happen.** Fitting a threshold on the data you score it on is how a
  useless metric looks useful. Every accuracy in this stream is leave-one-out.
- **Sample sizes are small and must be said out loud.** 19 south images and 13 north. A "95%" on 19
  points is one or two images from being ordinary, and `sat` winning on south is more likely an
  artifact of coloured junk in the rejects than a real discriminator.
- **A metric that cannot beat guessing the majority class is not a metric.** Report it as such.
- **Never regenerate what the user threw away.** The tooling reads every bucket before choosing
  seeds.

## Future intent this plan must not trim

- **The point is automated generation selection**, not a better ruler for its own sake. If no metric
  works, the honest answer is that a human must look — and the deliverable becomes making that cheap
  (contact sheets, sorting tools), not pretending otherwise.
- **A learned evaluator is the obvious next step and needs more data.** 63 labelled images is enough
  to falsify a metric and nowhere near enough to train a classifier. `art eval-data` exists to grow
  the set.
- **Whatever lands here feeds the LoRA streams.** Generation selection is the consumer; the east
  pipeline is only how the images get made.
