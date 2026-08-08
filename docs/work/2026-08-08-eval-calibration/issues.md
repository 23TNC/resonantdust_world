# Issues — eval calibration

_A problem hit, with what it cost and how it was found. Findings live here; decisions live in
[`forks.md`](forks.md)._

## I1 — every LoRA generation was selected on output the pipeline never produces {#i1}
_2026-08-08 · the user's question, run to its conclusion_

kohya's `--sample_every_n_epochs` writes 5 images per generation on a pinned prompt/seed file. They
do **not** enter training — verified: dataset and latent cache hold at 701/701/701 for the whole
run, samples land in `output_run20/sample/`, and zero sample images appear in the dataset tree. But
they *are* the whole of evaluation.

And they are **txt2img with no ControlNet**. Six east wolves from `r20g07`, no control:

- **2 of 6 are not animals** — abstract colour blocks; grey diagonal stripes
- **1 is a sprite sheet** — a grid of small wolf heads and diamonds
- **1 is a cropped bust** filling the frame
- **2 are standing wolves** on coloured backgrounds

`mean iou_ref 0.505 · mean aspect 1.05` against the real wolf's **2.08**. Everything is square,
because txt2img composes to the canvas rather than to a sprite. **None is in the convention.**

**So every generation choice on record — run-10 g08, run-20 g07 — was made by looking at images like
these**, while the pipeline always supplies a control. Those are different questions.

It also explains the day's whiplash: g10 "a loaf", g08 "close", the six-seed sweeps hopeless — then
`r20g07` through the real pipeline produced a clean three-view wolf. I recorded that as *my* sweep
being the wrong instrument ([east-pipeline I5](../2026-08-08-east-pipeline/issues.md#i5)). True, and
too narrow: **kohya's samples use the same instrument.**

## I2 — `iou_ref` does not predict human judgement {#i2}
_2026-08-08 · measured against 63 hand-labelled sprites_

Leave-one-out accuracy against the user's `good-*`/`bad-*` sorting, versus always guessing the
majority class:

| | n | baseline | `iou_ref` | `d_aspect` | best |
|---|---|---|---|---|---|
| east | 31 | 84% | **84%** | 90% | `d_aspect` 90% |
| south | 19 | 74% | **63%** | 63% | `sat` 95% (suspect — [I3](#i3)) |
| north | 13 | 62% | **38%** | 46% | `fill` 77% |

**`iou_ref` never beats the baseline, and on north at 38% it is anti-correlated** — it prefers the
sprites the user rejected.

**The mechanism was predicted and not measured.**
[east-pipeline F4](../2026-08-08-east-pipeline/forks.md#f4) says in as many words: "a correctly
shaped bear painted in the wrong colours scores perfectly". South's rejects have the *right*
silhouette — the control guarantees it — and a broken face. A silhouette statistic cannot see a
face, so it ranks a faceless sprite above a good one whose outline wobbles slightly.

**What this does NOT retract.** `iou_ref` compared *methods in aggregate* over 18 cells, where
S0 0.731 → S1 0.852 was driven by silhouettes that were genuinely the wrong animal — a thing it can
see. Those results stand. What it cannot do is rank sprites *within* a method, which is precisely
what generation selection needs. **Aggregate method comparison and per-image selection are different
jobs and one metric does not do both.**

## I3 — the metric that "wins" on south is probably an artifact {#i3}
_2026-08-08 · recorded so it is not built on_

`sat` scores 95% leave-one-out on south — the highest number anywhere in this stream — with good
mean 6.02 against bad 10.10. **Do not trust it yet.**

- **n = 19.** Two images changing bucket moves it to ~84%.
- **There is no mechanism.** "Is this wolf's face coherent" has no reason to be a saturation
  question. The likely story is that several rejects carry coloured junk (the red eye and purple
  slashes on one armadillo-era sprite, coloured plates elsewhere), so `sat` is detecting *artifacts*
  rather than the failure the user was actually judging.
- It also **inverts against east**, where good is *more* saturated than bad (9.23 vs 5.88). A
  discriminator that flips sign between directions is describing the sample, not the property.

[P1](todo.md) grows the set before anything is built on this.

## I4 — the control image sets the generation resolution, and it is 512 {#i4}
_2026-08-08 · found while investigating why south fails_

The i2i latent comes from `VAEEncode` of the **control image**, so the control's size *is* the
generation size. The silhouette bank is **512×512**. SDXL is native at **1024**. And the two
directions are not comparable within that frame:

```
Wolf_Timber/e   subject bbox 510x243   spans the FULL width
Wolf_Timber/s   subject bbox 202x450   a 39%-wide column
```

So a south frontal face is drawn in roughly a **100px box at 512** — half the base's native
resolution, at the scale where SDXL reliably mangles faces. East survives because its subject spans
the whole frame and its head is a large profile.

**Generating at 1024 fixes it.** Same three seeds: at 512 the face is a splayed white mask (the
failure the user rejected 12/12), at 1024 it is a wolf face with two eyes, a muzzle and a dark nose.

**A hypothesis of mine that was WRONG, checked first:** that south's control carried more interior
edge detail which ControlNet was tracing as outline. Measured — south's interior-edge fraction is
**20%, the same as east's 20%**. Not the cause.

**`--frame-fill` (crop the control to its subject, re-square) does not help**, and drags the source
plate's border into frame as black bars and red streaks. Implemented, measured, off by default.

**The gap that matters: the LoRA was trained at 768** — verified in `train_run20.sh`
(`--resolution="768,768"`) and in the latent cache names (`_0768x0768_sdxl.npz`). The 1024 datasets
on the box are from the older quad-only era and were not used. So the LoRA's resolution, the base's
resolution and the pipeline's resolution are **three different numbers**, and 768 — the only one
the LoRA has ever seen — is the one nobody has tested. [P2](todo.md).

## I5 — the direction asymmetry, in the user's own labels {#i5}
_2026-08-08_

```
east    good 26 / bad  5     84% acceptable
north   good  8 / bad  5     62%
south   good  5 / bad 14     26%
```

Consistent with every earlier finding — the design doc calls side profile "in-distribution, *easy*"
and front/back "*hard*" — but this is the first time it has been quantified **by the user's own
judgement on pipeline output** rather than by a proxy metric or by eye on bare txt2img.

South's 26% is largely [I4](#i4): those labels were collected on 512-generated sprites. The set will
need re-labelling once the resolution question is settled, and the current south numbers should be
treated as measuring *the 512 pipeline*, not the method.
