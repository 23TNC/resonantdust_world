# Issues — findings that shaped the plan

## I1 — At `dn=1.0` the template's i2i latent is inert; it is a silhouette source only

The pipeline VAE-encodes the template into the start latent *and* derives a ControlNet edge map from
it. With the measured defaults now at `dn=1.0` ([`generate.py`](../../../bin/lib/generate.py)) the
sampler fully destroys that start latent — **only the edge map survives**. Consequence for this
stream: "remove the template" is a narrower change than it sounds. Nothing needs to replace the
latent (an `EmptyLatentImage` is equivalent); only the silhouette needs a new source. This is why
P1 is small and P2 carries the weight.

## I2 — A template generalizes within a body-plan family, but not across one

Measured 2026-07-25 by holding the wolf template and changing only the species prompt
(`.staging/species-test/species.png`, 2 species × 2 seeds × e/s/n):

- **Leopard — success.** Re-skinned to genuinely feline features (rounded ears with pink inner ear,
  cat muzzle, banded tail, rosettes), not a recoloured wolf. Body plan matches: four legs, long
  tail, similar mass distribution.
- **Brown bear — failure.** Bear-face-inside-torso on the front views, a pale near-featureless blob
  on east, and a spurious tail the animal does not have. Body plan mismatch: bulky, tailless,
  different head carriage — and `cn=0.20` still forces the wolf outline onto it.

The unit of reusable control art is therefore the **body-plan family**, not the species. This is
what makes a finite control source viable ([forks F1](forks.md#f1)) and it sets P2's family table.

## I3 — Known template-free failure modes to hold the baseline against

From the txt2img + LoRA runs (`.staging/run3-samples/epoch7-gen/`), the failures the P0 gate must
catch:

- **wolf east collapses to a bust** — a head/shoulders portrait instead of a full body. Detectable
  by `aspect` against the reference sprite (a bust is far squarer than a side profile).
- **bear east goes blobby** — hunched, legless mass. Detectable by `solidity` + `aspect`.
- **background bleed** — the blue-grey and grey plates seen on tiger south/north. Detectable by `bg`.
- **scale drift** — bear huge, cat small in the same batch. Detectable by `fill`.

All four already have metrics in [`lora_eval.py`](../../../bin/lib/lora_eval.py); P0 only has to set
the thresholds.

## I4 — Template-free fails on **south only**; east and north are already production-grade

P0 baseline, 10 species × e/s/n, one seed, `rd_quadruped_e07` @ 0.85
(`.staging/tf-baseline/scores-{none,template}.csv`). Valid = the [F4](forks.md#f4) gate.

| mode | valid | east | south | north | mean score |
|---|---|---|---|---|---|
| `none` (txt2img + LoRA) | **25/30 (83%)** | 10/10 | **5/10** | 10/10 | 71.6 |
| `template` (wolf art) | 29/30 (97%) | 10/10 | 9/10 | 10/10 | 78.8 |

Mean aspect error by direction, template-free: **east 18.4% · south 153.7% · north 17.9%**. The
entire template-free deficit is one direction. East/north are statistically indistinguishable from
templated (18.4 vs 15.0; 17.9 vs 15.5) — control art buys ~nothing there.

**What south actually fails at is FACING, not quality.** The five failures (bear, cat, fox, pig,
horse) generate a *side profile* when asked for a front view — aspect 1.36–2.02 against references of
0.28–0.84. The LoRA's `rd_south` binding is too weak to beat the base model's preference for drawing
an animal side-on, and with no control image nothing corrects it.

**Consequence for the plan:** P2's silhouette bank does not have to serve all three directions. A
south-only control source would recover ~all of the gap, which makes the corpus approach cheaper than
budgeted. Do not over-build east/north control.

## I5 — The `bg` metric conflated scenery bleed with a sprite touching the frame edge

Found while validating the F4 gate: `bg = 1 − (fraction of border pixels that are subject)` rejected
a **hand-approved** bear-north (user: "perfect") at `bg=0.84`, purely because the bear fills the frame
vertically so 16% of the border is bear — on a pristine white plate.

Two unrelated properties were being measured as one: *is the plate keyable* (fatal if not) and *does
the subject touch the edge* (normal, often desirable). Fixed by adding **`bg_uni`** — the colour
uniformity (mean per-channel std) of the **non-subject** border pixels, which is what
`remove_bg_floodfill()` actually depends on. `bg` is retained for reporting.

Verified: bear-north now `bg=0.84 / bg_uni=0.96` → PASS, while both known failures still fail on
aspect, and a scenery-bleed case would still fail on `bg_uni`.

## I6 — The south-view failure is seed-dependent, not deterministic

The [I4](#i4) baseline was **one seed per cell**, and reported template-free bear-south at 136.6%
aspect error (FAIL). Re-running the same species template-free across three seeds gave 43.5%, 46.3%
and 14.9% — **all passing**.

So "5/10 south failures" is a per-seed failure *rate*, not a per-species verdict: the model can draw
a correct front view for these species, it just often doesn't. Two consequences:

1. **I4's numbers understate template-free's ceiling** and overstate its floor. Treat them as
   "probability a single roll succeeds", not "this species cannot be generated".
2. **This is the argument for P3.** If failure is stochastic, sampling several candidates and
   screening converts an unreliable generator into a reliable one at the cost of GPU time — which
   is exactly the "quantity + screening replaces precision" stance in the README.

A stronger baseline would use several seeds per cell and report a rate with an interval. Not re-run
here (it is ~10× the GPU time for a number whose decision — build P3 — is already made), but any
future comparison of control modes should be multi-seed to be sound.
