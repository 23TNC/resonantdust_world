# Issues — direction consistency

_A problem hit, with what it cost and how it was found. Findings live here; decisions live in
[`forks.md`](forks.md)._

## I1 — `generate.py`'s `STYLE` is English prose; every style-only LoRA is trained on tags {#i1}
_2026-08-08 · found running r20g07 through the pipeline_

[`generate.py:58`](../../../bin/lib/generate.py) appends a fixed boilerplate to every positive
prompt, for every LoRA:

```
oblique top-down view game creature sprite, {face}, flat cel shading, bold dark outline,
hand-painted 2D game art, cartoon, on a plain solid white background
```

`rd_diremph_anima_r20_g07` was trained on tag lists and has **never seen those words**:

```
rd_south, rd_south, rd_south, rd_style, rd_animal, rd_quadruped,
front view, facing the viewer, facing forward, single creature, full body
```

Run side by side on the same LoRA, seed family and knobs — the only difference being the prompt
text — the prose form produced **cyan rim-light and peach hindquarter artifacts in all three
directions**, plus a **hollow unfilled tail** on south. The trained-tag form produced a clean
grey/white wolf in all three. Measured saturation over opaque pixels:

| | east | south | north |
|---|---|---|---|
| prose | 48.0 | 20.4 | 28.3 |
| trained tags | 4.4 | 6.4 | 4.1 |

The prose form's saturation is not merely different, it is *incoherent across directions* — 48 on
east against 20 on south, from one animal. The cyan and peach are the base model's rim-lighting
prior filling in for words the LoRA has no response to.

**Applies to every LoRA this project trains from here**, since the whole style-not-species thesis
produces tag-captioned models. Sheet: `.staging/r20g07_pipeline.png`.

## I2 — south and north are anchored on east, never on each other {#i2}
_2026-08-08 · read out of `generate.py`, then confirmed in the measurement_

`main()` forces `dirs = ["e"] + [d for d in dirs if d != "e"]` — east renders first through
`graph_hero`, and its output becomes `hero_name`, the IP-Adapter anchor for both other views via
`graph_ip`. South and north are therefore **siblings that never see each other**, each free to
drift from east in its own direction.

The measurement says they do, and in a specific shape: luminance rises **monotonically in render
order**, `133.9 → 146.5 → 158.8`, spread **24.9** against a corpus ceiling of **9.0**. A shared
anchor that carried identity would not produce a ramp; a per-view independent drift from a weak
anchor would.

## I3 — the anchor's `weight_type` is `"style transfer"`, which discards content by design {#i3}
_2026-08-08 · read out of `generate.py:480`_

```
"42": {"class_type":"IPAdapter", "inputs":{..., "weight":IP_WEIGHT, "weight_type":"style transfer", ...}}
```

`style transfer` exists to carry *style* while **discarding composition and content**. "This is the
same wolf — same markings, same value, same palette" is content. So the one mechanism in the
pipeline nominally responsible for cross-view identity is configured to throw identity away.

`IPAdapterAdvanced` (installed) exposes composition-carrying weight types, and
`IPAdapterStyleComposition` (installed) takes style and composition as **separate** references —
which matches the actual division of labour here, where composition should come from the template
and identity from the hero. Neither was reachable in the 11 GB era's graph budget.

## I4 — the sprites are internally consistent in OUTLINE and we did not earn it {#i4}
_2026-08-08 · the user's observation, confirmed_

> *"I suspect we are hard locked to our control net."*

At `cn 0.5 / cn-end 0.9` the silhouette is the authored `template.{e,s,n}.0.png` triple. The three
generated views line up in shape because **the templates line up**, not because the model holds
identity. Two consequences worth keeping in view:

- Any consistency win measured on **silhouette** is measuring the templates, not the pipeline. Only
  interior metrics — value, saturation, palette — say anything about this stream's changes.
- Every new creature needs an authored template triple. Real, and explicitly **out of scope**
  ([README](README.md)); the [`lineart-lora`](../2026-07-27-lineart-lora/README.md) stream is where
  that dependency gets removed.

## I5 — my six-seed sweeps were the wrong instrument, and I reported them as evidence {#i5}
_2026-08-08 · caught by the user_

> *"something is quite off in your 6 south wolf generations as none of them are even close."*

The sweeps were **bare txt2img with no ControlNet template**, at LoRA strength 0.85, on seeds
unrelated to the training samples — while kohya's own training samples apply the network at 1.0 on
seed 22. So the sweep asked the LoRA to *invent* the pose from nothing, which is not the job it was
trained for and not what the pipeline asks of it. The same checkpoint that looked poor across six
seeds produced a usable three-view set the moment it was given a template.

**The instrument, not the model, was failing.** Recorded because the failure shape is reusable: a
measurement run outside the pipeline the artefact ships through can be confidently wrong in the
model's favour *or* against it, and this one was against it.

Related: the corpus check that should have preceded those sweeps came after them. The south
convention draws the tail as **a single spike above the head** in 8 of 10 canines — so the
"tail became a second pair of ears" reading was describing the convention rendered badly (too wide,
split) rather than a hallucinated feature. Ground truth first, then judgement.

## I6 — EAST is the outlier; south and north already agree with each other {#i6}
_2026-08-08 · P0.4 baseline, 12 sets × 4 species × 3 seeds · **this corrects [I2](#i2)**_

[I2](#i2) said south and north are siblings that never see each other and are therefore "each free
to drift from east in its own direction". The baseline says the opposite. Splitting each set's
spread into *east versus the pair* and *south versus north*:

```
east is the EXTREME (max or min) in   11 of 12 sets
mean |east - mean(south, north)|  =  19.3
mean |south - north|              =   7.7      <- the corpus ceiling is 9.0
ratio                                 2.51x
|south - north| clears the ceiling in 8 of 12 sets
```

**South and north are already consistent with each other**, at 7.7 mean against a ceiling of 9.0.
Nearly all the measured drift is east standing apart from both.

**The mechanism this points at is sharper than "weak anchoring".** East is the one direction
generated by a *different graph*: `graph_hero` (txt2img + ControlNet, **no IP-Adapter**), while
south and north both go through `graph_ip` (**IP-Adapter applied**). So the pipeline is comparing
output from two different graphs and calling the gap inconsistency. The IP-Adapter is not failing
to carry value between south and north — it is carrying it *well*, which is exactly why they
cluster, and why the un-adapted image is the one that stands out.

**What it changes.** [P2](todo.md)'s premise was "anchor every view on every other view". That is
now the *second* priority: south and north do not need each other, they need east to be produced
the way they are. The first move is to put east through the same graph — self-anchored, or
anchored on a neutral reference — so all three views are drawn from one graph rather than two.

**Why I got it wrong.** I2 was written from one wolf set and a code read. Three views of one animal
cannot distinguish "s and n drift apart" from "e stands apart" — both produce a large spread. It
took 12 sets to separate them, which is what the pinned set exists for.

## I7 — the gate quarantines a leaf AFTER reporting its path, silently dropping cells {#i7}
_2026-08-08 · found when the baseline returned 35 sprites for 36 cells_

`report_candidates` moves a whole variant leaf into `_rejected/<leaf>/` when any one direction
fails the structural gate — and it does so *after* `generate.py` prints `wrote <path>`. The harness
reads that path, so by measurement time the file has moved: `wolf/9103` measured as a two-direction
set and its east was never counted.

**Worse than one lost cell: it biases the population.** Sets that survive intact are exactly the
sets that passed the gate, so an unfixed harness measures cross-direction consistency *on the
subset that already generated cleanly* — and reports it as the baseline.

**Fixed** by `resolve_written()`, which looks in the quarantine before giving up and prints
`gate-rejected (measured from _rejected/)` when it finds it there. Added `--measure-only` to
re-measure a label's sprites from disk without regenerating; re-running it recovered `wolf/9103`
east (lum 146.0) and completed all 12 sets. The aggregate did not move (mean 23.4 either way,
because that cell's east happened to fall between its own south and north) — but it would have on a
different draw, and the next label to be measured is the one it would have silently biased.
