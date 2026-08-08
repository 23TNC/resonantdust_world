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
