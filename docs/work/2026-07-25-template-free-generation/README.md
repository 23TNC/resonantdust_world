# Template-free generation — generalized animals without hand-authored control art — 2026-07-25

_Component: [`dev/art`](../../components/dev/) · `bin/lib/generate.py` (the ComfyUI pipeline) +
`bin/lib/lora_eval.py` (the QA harness). Phases in [`todo.md`](todo.md); decisions in
[`forks.md`](forks.md); findings in [`issues.md`](issues.md). Downstream of the `rd_quadruped`
LoRA trained 2026-07-25 (run-3 epoch 7)._

## The problem (user, 2026-07-25)

`bin/art generate` **cannot produce a species we have not already drawn.** `load_template()`
`SystemExit`s unless `textures/<kind>/template.<dir>.<part>.png` exists, and that template drives
both the ControlNet edge map and the i2i latent. So the pipeline is *reproductive*, not
*generative*: to get a new animal you must first author the art the generator was supposed to save
you from. The whole tooling assumes a provided control image; there is **no generalized path**.

## Why this is now solvable — the LoRA changed the constraint

The template historically did two jobs. Both have weakened:

1. **It supplied the style.** No longer — the `rd_quadruped` LoRA carries the look. That is what let
   the measured defaults move to `dn1.0/cn0.20/cn_end0.30`
   ([`generate.py`](../../../bin/lib/generate.py)): ~3.4× the seed-to-seed variety of the old recipe.
2. **It supplied the i2i latent.** No longer — **at `dn=1.0` the template's latent is fully
   destroyed** ([issues I1](issues.md#i1)). The template is *already* reduced to one thing: a
   silhouette source for ControlNet.

And the LoRA holds body plan on its own: at **`cn=0.00`** (ControlNet entirely off) it still produced
a correct, on-style, lying-down wolf — aspect 1.92 against the template's 2.12. Pure txt2img + LoRA
generated usable wolf/tiger/bear/cat across e/s/n. So control art is no longer *required* for a
plausible sprite; it is an accuracy aid.

## What the evidence says about how far a template generalizes

Measured this session by swapping the species while keeping the wolf template:

- **Leopard succeeded** — fully re-skinned to feline features (rounded ears, cat muzzle, banded
  tail, rosettes), not merely recoloured.
- **Brown bear failed** — bear-face-inside-torso on the front views, blobby east, a spurious tail.

A template therefore generalizes **within a body-plan family and not across it**
([issues I2](issues.md#i2)). That is the load-bearing finding: the unit of control art is a *family*,
not a *species* — which is what makes a finite, non-authored control source possible.

## Design stance

**Retire the hard template requirement; make the control silhouette a pluggable source, and make
failure cheap instead of impossible.** Three commitments:

1. **Control becomes a mode, not a file.** `--control {template,corpus,family,none}` — the current
   hand-authored path survives as one mode, never removed (explicit art must always win).
2. **The corpus IS the template library.** `.staging/quad-lora-train` holds 459 real, on-model
   sprites across 132 species × e/s/n. Their silhouettes are ready-made control images, already in
   the target style, at zero authoring cost. A new species borrows the silhouette of its nearest
   body-plan relative. This is the reason the stream does not end in "draw six more templates".
3. **Quantity + screening replaces precision.** A generalized generator will fail often. That is
   acceptable *if* failure is detected automatically: [`lora_eval.py`](../../../bin/lib/lora_eval.py)
   already scores blobs / bg / fill / aspect / solidity against real reference sprites. Generate N,
   score, keep the survivors — the flywheel the user described (generate → hand-pick → retrain).

## Future intent this plan must not trim

The stream is scoped to quadrupeds because that is the LoRA we have, but the control-source
abstraction is deliberately body-plan agnostic: the **biped** and **humanoid** LoRAs are planned
next, and each will want the same `corpus`/`family` control path against its own silhouette bank.
Do not special-case quadrupeds in the interface. Likewise the screening layer is the input to the
**self-training flywheel** (curated generations fed back as training data) — it must emit a
machine-readable score per candidate, not just a human-eyeball montage.
