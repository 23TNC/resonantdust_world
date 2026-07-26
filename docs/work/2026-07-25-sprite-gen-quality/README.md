# Sprite-gen quality — raise the ceiling on template-free generation — 2026-07-25

_Component: [`dev/art`](../../components/dev/) · `bin/lib/{generate,lora_eval,silhouette_bank,prep_train}.py`
+ the `rd_quadruped` LoRA. Phases in [`todo.md`](todo.md); decisions in [`forks.md`](forks.md);
findings in [`issues.md`](issues.md). Successor to
[`2026-07-25-template-free-generation`](../2026-07-25-template-free-generation/README.md) (DONE 22/22)._

## Why this stream exists

The generator works — arbitrary quadrupeds, no authored art, 15/15 valid on species absent from
training. This stream is about the **quality ceiling**, triggered by the anteater: generated on
`family:pachyderm`, its east view is genuinely good (long tapered snout, hunched back, bushy tail,
shoulder stripe) while south and north collapse into a cape-like blob. User: *"doesn't quite look
right in any view."*

## The ordering argument — fix the ruler first

The instinct is to go straight at the training data, and the data fixes probably *are* the biggest
single lever. But they must not go first, because **the measuring instrument is broken in both
directions**:

- **False positive** — the blobby anteater south/north scored `d_aspect` 13.2% / 4.1% and **passed**.
  The [F4 gate](../2026-07-25-template-free-generation/forks.md#f4) measures bounding-box proportions,
  not whether a shape reads as an animal.
- **False negative** — a good horned oryx was quarantined on `solidity=0.349` against a 0.35
  threshold. It failed by **0.001**, because horns enclose empty bbox area and read as "wispy".

Every downstream comparison — new LoRA vs old, `auto` control vs `family`, curated-flywheel vs not —
is scored by that instrument. Re-tuning the data and then measuring with a ruler that both passes
blobs and rejects antlers produces a number nobody should trust. **So P0 fixes the gate, and
everything after it is measured honestly.** This is the one sequencing decision in the stream that is
not negotiable.

After that the order follows cost-vs-leverage: cheap CPU-only fixes that improve every generation
(control selection), then the expensive GPU work (data + retrain), then the flywheel that depends on
all of it.

## What we think is wrong, in likely-impact order

1. **Training data is blurred and unnormalised** (highest ceiling, [I1](issues.md#i1)).
   `prep_train.py` upscales with **LANCZOS**, which blurs edges — and most sources are 256px or less,
   so *nearly every training image* taught the model soft boundaries. The user's framing is the key
   one: this art is effectively **vector graphics rendered as bitmaps**, so an edge-preserving
   upscale *reconstructs* the crisp original rather than inventing detail. Separately, subject
   **scale is not normalised** (bear nearly fills the frame, cat is tiny), so the LoRA learned that
   scale is arbitrary and output scale drifts.
2. **Control selection is a hand-written table** ([I2](issues.md#i2)). `FAMILY_REP` maps 11 families
   to one representative each, chosen by hand. The anteater got Elephant because someone typed
   `pachyderm -> Elephant`. A shape-similarity search over all 396 bank silhouettes should beat a
   human guess and removes the table.
3. **The screen is proportion-based, not shape-based** (above). Candidate fix: **IoU between the
   generated silhouette and the control silhouette it was given** — an output that ignored its
   control (the anteater blob) scores low, which is far closer to a semantic check.
4. **Cross-direction coherence is ~6× looser than real art** — 0.169 generated vs 0.028 for real
   same-animal sets (0.565 = different animals). Identity holds; tightness does not.
5. **The flywheel has never run.** Every piece exists — mass generation, screening, quarantine,
   scoring — but no generate → curate → retrain cycle has been executed once.

## Design stance

- **Measure before and after, multi-seed.** The predecessor's baseline was single-seed and reported a
  failure *rate* as if it were a species verdict ([I6 there](../2026-07-25-template-free-generation/issues.md#i6)).
  Every comparison in this stream reports a rate over ≥3 seeds.
- **Prefer removing hand-tuned knobs to tuning them.** The family table and the solidity constant are
  both human guesses standing where a measurement belongs.
- **The human stays the final filter.** Auto-screening catches geometry, not semantics — the anteater
  proves it. The gate's job is to cut the pile down to something worth a human glance, never to
  approve art unseen.

## Future intent this plan must not trim

The control-selection and screening work is deliberately **body-plan agnostic**: the planned
**biped**, **legless**, **arthropod** and **humanoid** LoRAs will each arrive with their own
silhouette bank and must reuse `--control auto` and the same gate untouched. Do not special-case
quadrupeds. Likewise the pose ceiling
([I7 there](../2026-07-25-template-free-generation/issues.md#i7)) is *not* in scope here — a stance
absent from the corpus cannot be asked for, and the fix is a new bank, not a better gate.
