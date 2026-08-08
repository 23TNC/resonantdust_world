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
