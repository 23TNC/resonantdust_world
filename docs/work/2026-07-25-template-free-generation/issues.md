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
