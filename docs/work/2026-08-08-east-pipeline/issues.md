# Issues — the east pipeline

_A problem hit, with what it cost and how it was found. Findings live here; decisions live in
[`forks.md`](forks.md)._

## I1 — one authored template exists, and it is a wolf {#i1}
_2026-08-08 · the user's observation, run down to its cause_

> *"this bear looks like a wolf"*

```
$ find textures -name "template.*.png" | xargs -n1 dirname | sort -u
textures/pawn/animal/wolf
```

**One.** `textures/pawn/animal/wolf/template.{e,s,n}.0.png` is the only control art in the
repository, and `resolve_control`'s default mode is `template`. At `cn 0.5 / cn-end 0.9` the control
does not suggest a shape, it **dictates** one — which the predecessor recorded approvingly as
"[outline consistency is already solved](../2026-08-08-direction-consistency/issues.md#i4)" without
noticing that it is solved *as a wolf*.

So every species generated through this pipeline is a wolf silhouette wearing that species' colours.
The bear is the clearest case because a bear's proportions are furthest from a canid's, but it
applies to all of them.

**I made it worse and recorded it as a feature.** `consistency_set.json` points all four subjects —
wolf, bear, fox, anteater — at the wolf template deliberately, so the silhouette stays constant and
the interior numbers compare across cells. That reasoning is sound *for measuring interiors* and it
holds species fidelity at exactly zero. The note in the file says the bear and anteater "come out
wolf-shaped, and that is fine for the question being asked". The question being asked was too narrow.

**The design doc predicted the failure mode.** `sprite-gen-plan.md` § reference-driven mutation:
"ControlNet preserves the reference outline, so mutating across very different body-plans distorts
proportions (wolf→lion great; **wolf→cat came out leggy**)". Its stated mitigation is "references
per body-plan so shapes start close" — which is [P1](todo.md)'s S1.

## I2 — the fix may already be implemented and has never been run {#i2}
_2026-08-08 · found by reading the pipeline before planning against it_

`bin/lib/silhouette_bank.py` exists, is **built**, and holds **133 species × e/s/n** under
`.staging/silhouette-bank/`. Its docstring states the thesis directly:

> The template's only surviving job is to give ControlNet a silhouette… We already own 459 real,
> on-model sprites across 132 species x e/s/n in the training corpus — so **the corpus IS the
> template library**, at zero authoring cost and already in the target style.

Every species this stream plans to test has an `e.png` in it — verified for `Wolf_Timber`, `Tiger`,
`Fox_Red`, `AEXP_BlackBear`, `Bear`, `Deer`, `Pig`, `Cow`.

And `resolve_control` already dispatches **three** non-template modes:

| mode | what it does |
|---|---|
| `corpus:<Species>` | that species' own real silhouette as the control |
| `family:<f>` | a representative species per body-plan family |
| `auto` | **already two-stage** — a template-free probe, then the nearest bank silhouette by measured body plan |

**None of them has ever been run in a measured comparison.** A `resolve_control` docstring still
describes the corpus modes as "P2, not yet wired" while the code four lines below dispatches them,
so the comment is stale — a small thing that would have made a reader believe the capability was
absent. Recorded because it nearly made *me* plan to build what is already there.

**Consequence for the plan:** S1 is a flag, not a feature. It gets measured first, before anything
is built, because the cheapest candidate fix deserves to set the bar the multi-stage work must beat.

## I3 — `iou_ref` is the only metric that can see this defect {#i3}
_2026-08-08 · why the predecessor's numbers all approved of a wolf-shaped bear_

Every metric the predecessor used — luminance spread, saturation, coverage, the structural gate,
`iou_control` — scored the wolf-shaped bear as fine. `iou_control` scored it *well*, because the
sprite obeyed the control it was handed. `lora_eval.iou_ref` is the exception, and its docstring
draws the distinction itself:

> that compared against the CONTROL image the generator was handed, so **a wrong control faithfully
> obeyed scored HIGH**. This compares against ground truth, so it can see what no bounding-box
> statistic can — a sitting wolf overlaps a lying wolf poorly, and a framed bust overlaps a full
> body poorly. **Only defined for corpus species.**

"Only defined for corpus species" is precisely why this stream starts with trained animals: they are
the only ones where the ruler works, so they are where the method gets calibrated. For
[P4](todo.md)'s untrained species `iou_ref` is undefined and the eye is the judge — which is a real
limitation of the plan, stated here rather than discovered later.

## I4 — a concurrent session stubbed this file out mid-write {#i4}
_2026-08-08 · noticed immediately, content restored_

While this stream was being authored, another session wrote a placeholder over `issues.md` — *"Stub
created by the spawn-authority session to keep the docs tree green while this stream was
mid-creation"* — discarding I1–I3. Rewritten from the same evidence.

Harmless here because the loss was visible in the same turn and nothing had been committed. Recorded
because the failure is silent by design: a docs-green hook that *writes* content will overwrite an
in-flight file without a conflict, and a session that did not happen to re-read the file would have
committed the stub over its own findings.

## I5 — `iou_ref` penalises CORRECT species features the reference individual lacks {#i5}
_2026-08-08 · found at P1's S1, where the metric and the eye disagreed_

Deer is the only species S1 barely moved: `iou_ref` 0.700 → 0.709, and at seed 4101 it scored
**0.693** — the worst cell in the run. The sheet says the opposite. S1's deer has **antlers, thin
separated legs and a dappled coat**; S0's deer is a tan wolf shape with a deer's head stuck on.

The cause is in the reference, not the sprite. **The real `Deer` corpus sprite is a spotted doe with
no antlers.** The generated buck's antlers fall entirely outside the reference silhouette, so they
count as union-without-intersection and drag IoU down — while being *more* correct as "a deer", not
less.

**So `iou_ref` measures agreement with one specific individual, not with the species.** It is still
the right primary metric — it is the only one that catches a wolf-shaped bear ([I3](#i3)) — but it
has a known blind spot in the *opposite* direction from the one it was chosen for, and deer is the
cell where that shows.

**Consequences, recorded so no later phase treats deer's number as a shape failure:**

- Deer's low score must **not** be optimised away. A method that raises it by removing antlers is
  worse, not better.
- The same trap applies to any species whose corpus sprite is one individual of a dimorphic or
  variable species. Deer is the one in this set; there will be others in [P4](todo.md).
- `d_aspect` is the safer read for deer, and it agrees with the eye: mean |aspect error| 21.9 → 17.2.

**This is the sixth time in this project that the images have overturned a number**, and the first
where the number was wrong for a reason worth keeping rather than a defect to fix.

## I6 — `--control auto` declined 18/18, and its fallback is NO CONTROL {#i6}
_2026-08-08 · P1's S3 · **the probe stage is the broken part, and this is the finding P2 needed**_

S3 scored `iou_ref` **0.556** — worse than the incumbent wolf template's 0.731 — with `d_aspect`
near **−52 on almost every cell** and gate failures on **11 of 18** (blobs up to 40). Uniform
numbers across six very different species meant something structural, so the control decision was
recorded per cell rather than inferred:

```
10x  auto DECLINED (best match Gorilla    0.57 < 0.65)
 4x  auto DECLINED (best match Gorilla    0.58 < 0.65)
 2x  auto DECLINED (best match Orangutan  0.61 < 0.65)
 1x  auto DECLINED (best match Gorilla    0.52 < 0.65)
 1x  auto DECLINED (best match Orangutan  0.60 < 0.65)
```

**Eighteen of eighteen declined, and the nearest bank match was a primate every single time** — for
wolf, bear, fox, deer, elephant and tiger alike.

**So S3 never measured what its name says.** `resolve_auto` declines below `AUTO_MIN_MATCH = 0.65`
and `generate.py` then proceeds *with no ControlNet at all*. Every S3 number describes **bare
txt2img**, not probe-then-match. The scores are real; the label was wrong.

**Two separate defects, and they should not be conflated:**

1. **The probe stage does not produce a side-profile quadruped.** Template-free txt2img with this
   LoRA yields something upright and compact — which is why it matches *Gorilla* and why
   `d_aspect ≈ −52` says the result is half as long as it should be. The matcher is behaving
   correctly: it is reporting that the probe looks like nothing in the bank, because it does.
2. **The decline fallback makes things worse, not safer.** Falling back to *no control* removes the
   only thing holding the composition together, which is how 11 of 18 cells fragmented. Falling back
   to `family:<plan>` or the template would degrade gracefully instead.

**Why this is the most useful result of the phase.** It is the direct measurement of the naive form
of the user's proposal — *generate the silhouette first, then texture it*
([P2](todo.md)). Generating the silhouette from an unconstrained pass **does not work**, and now the
reason is measured rather than suspected: the model will not produce our side-profile convention
without something already holding the pose. P2's stage 1 therefore cannot be "txt2img and see"; it
needs its own constraint, and identifying that is the phase's real question.

## I7 — for untrained species, colour transfers and SHAPE does not {#i7}
_2026-08-08 · P4 · the failure trained species could not show_

Four species absent from both the corpus and the bank, generated from their family representative's
silhouette (`S2a`, the only method that can serve an unknown animal). All 12 cells passed the
structural gate — `blobs = 1`, `bg_uni ≈ 1.000` — so nothing *broke*. They are simply the wrong
animals.

| species | family rep | result |
|---|---|---|
| **okapi** | Horse | **works** — brown body, white leg stripes, dark legs, recognisably an okapi in all three seeds |
| anteater | Bear | a pale bear-shaped blob with a slightly pointed nose. **No long snout, no bushy tail.** |
| armadillo | Pig | a pale pig. **No armour bands, no plates.** |
| warthog | Pig | a pale pig. **No tusks, no heavy head.** |

**The rule that explains all four:** at `cn 0.5` the control *dictates* the silhouette, so **the
prompt can only repaint the interior**. Okapi succeeds because an okapi genuinely is a horse-shaped
animal with stripes — its identity is **colour**, and colour is the one thing the prompt still
governs. Anteater, armadillo and warthog fail because their identity is **shape** — a snout, a
banded shell, tusks — and the borrowed silhouette forbids exactly that.

Trained species could never expose this, because each had *its own* silhouette available. It is the
first defect in the stream that only appears where the product actually lives.

**Two lesser findings from the same run:**

- **The pose convention is inherited too, not just the outline.** Okapi at seeds 4102/4103 comes out
  **standing upright** rather than in the corpus's crouching side profile — because the `equine`
  representative (Horse) stands. Borrowing a silhouette borrows its posture.
- **Too-pale is much worse here.** Untrained luminance runs **143–194** against the corpus band of
  62–121; only okapi (69–94) lands inside it. The predecessor's pale-output defect is amplified when
  the model has no learned example of the species to anchor value.

## I8 — one seed systematically reverts to the naturalistic STANDING pose {#i8}
_2026-08-08 · the user's observation on the wolf grid, run down across the set_

> *"we have a seed that really wanted to stuff the legs back in there."*

Confirmed, and the legs are a **symptom** rather than the thing. Signed aspect error against the
real animal (negative = more compact = standing upright rather than crouched):

| species | 4101 | 4102 | 4103 |
|---|---|---|---|
| wolf | −4.1 | +0.1 | **−31.0** |
| bear | −7.3 | −1.6 | **−34.3** |
| deer | −18.8 | +1.4 | **−31.3** |
| fox | −2.0 | −0.4 | −2.7 |
| elephant | −1.4 | −44.1 | −1.2 |
| tiger | −1.3 | −1.1 | −0.6 |

```
seed 4101: mean  -5.8   badly compacted (< -20%): 0/6
seed 4102: mean  -7.6   badly compacted (< -20%): 1/6   (elephant)
seed 4103: mean -16.9   badly compacted (< -20%): 3/6   (wolf, bear, deer)
```

**Seed 4103 breaks the pose convention on half the set**, and the three it breaks — wolf, bear,
deer — are exactly the species for which "standing quadruped" is the overwhelming prior in natural
images. Fox and tiger (long, low, lithe) and elephant resist it. So this is not a seed that likes
legs; it is a seed that reverts to the **naturalistic pose**, and separated legs are what that looks
like at the silhouette's bottom edge.

**A measurement of mine that FAILED, recorded so it is not retried.** The obvious metric is
"connected runs along the bottom strip", on the reasoning that the convention means an unbroken
bottom edge. **The real corpus sprites score 2–4 on it, not 1** — actual on-model animals do have
gaps under them — so the metric has no clean threshold and cannot support a conclusion. `d_aspect`
is the honest instrument here.

**The consequence that matters, and it is a hole in [P1](todo.md)'s S5 selector:** a standing wolf
has `blobs = 1` and a perfectly uniform plate, so the **reference-free gate selector cannot see this
failure at all**. It would happily pick 4103. Only `d_aspect` catches it, and `d_aspect` needs the
real sprite — which is exactly what untrained species do not have.

**And the obvious reference-free substitute does not work either.** An aspect floor sounds
promising until measured: over 200 corpus species the east aspect runs min 0.54, p5 0.74, median
1.59, p95 2.82, and **55 of 200 sit below 1.30**. A floor that rejected standing wolves would
reject a quarter of the real corpus. Pose-convention checking for unknown species is therefore an
**open problem**, not a knob — and it is the second thing (after [I7](issues.md#i7)) that the
untrained path needs and does not have.
