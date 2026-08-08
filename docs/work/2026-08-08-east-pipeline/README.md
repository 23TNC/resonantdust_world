# The east pipeline — how many stages does a good sprite take? — 2026-08-08

_Component: [`dev/art`](../../components/dev/) · `bin/lib/{generate,silhouette_bank,lora_eval}.py`.
Plan in [`todo.md`](todo.md); decisions in [`forks.md`](forks.md); findings in
[`issues.md`](issues.md). Successor to
[`2026-08-08-direction-consistency`](../2026-08-08-direction-consistency/README.md), parked on the
user's redirect. LoRA frozen at `rd_diremph_anima_r20_g07`._

## What the user asked for

> *"this bear looks like a wolf... and our rotational consistency is getting much worse. Lets take a
> step back. … investigate the best methods to generate east facing textures of animals we have
> training data for using the run20g7 lora. I would specifically like to investigate more stages in
> our pipelines. I suspect we should be able to generate our silhouette, and then texture that
> silhouette, for example. I'd like you to investigate other promising solutions as well. Once we
> have a decent method to handle this, we will attempt animals we do not have training data for …
> We are tackling east facing only."*

**East only. Trained species first. Stage count is the variable.**

## Why the bear was a wolf — the one-line cause

```
$ find textures -name "template.*.png" | xargs -n1 dirname | sort -u
textures/pawn/animal/wolf
```

**There is exactly one authored template in the entire tree.** Every sprite this project has pushed
through the pipeline has been shaped by the wolf's e/s/n silhouette, at `cn 0.5 / cn-end 0.9` —
strong enough that the silhouette *is* the template. A bear asked for through that template can only
come out wolf-shaped. The predecessor's pinned set made this worse by pointing all four subjects at
it deliberately, to hold the silhouette constant so the interior numbers stayed comparable; that
made the interior legible and species fidelity unmeasurable.

The design doc predicted it: reference-mutation "distorts proportions… wolf→lion great; wolf→cat came
out leggy" ([sprite-gen-plan](../../components/dev/scripts/art/plan/sprite-gen-plan.md)).

## Two things already exist and have never been measured

Found by reading before planning, not by assuming:

- **`silhouette_bank.py`, built, 133 species × e/s/n.** Its own docstring states the thesis: *"We
  already own 459 real, on-model sprites… the corpus IS the template library, at zero authoring
  cost and already in the target style."* Every candidate species below has an `e.png` in it.
- **`--control corpus:<Species>`** is wired through `resolve_control`, alongside `family:<f>` and
  **`auto`** — which is *already* a two-stage pipeline: a template-free probe, then the nearest bank
  silhouette by measured body plan. A docstring still calls the corpus modes "P2, not yet wired";
  the code says otherwise, and **neither has ever been run in a measured comparison.**

So the cheapest candidate fix for the bear is one flag, and the user's multi-stage instinct already
has a partial implementation to build on.

## The advantage this stream has that the predecessor did not: GROUND TRUTH

For a species with training data, **the real sprite exists**. `lora_eval.iou_ref` scores a generated
silhouette against that real corpus sprite — not against the control it was handed, which is the
distinction its own docstring draws:

> that compared against the CONTROL image the generator was handed, so a wrong control faithfully
> obeyed scored HIGH. This compares against ground truth.

That is exactly the failure we just shipped: the bear obeyed the wolf template faithfully and every
metric approved. So this stream measures **fidelity to a known answer**, where the predecessor could
only measure self-consistency. `d_aspect_signed` adds proportion with a sign, and the interior
metrics from the predecessor carry over for colour.

**The honest question this raises, answered up front:** if we already own the sprite, why generate
it? Because a method that cannot reproduce an animal it *has seen* will not invent one it has not.
Trained species are the calibration set, and they come with an answer key. Untrained species are the
actual product, and they are [P4](todo.md).

## What gets investigated

The user named silhouette-then-texture. It is one rung of a ladder, and the ladder is the plan:

| | stages | what it is |
|---|---|---|
| **S0** | 1 | wolf template + LoRA — the incumbent, and the control |
| **S1** | 1 | the species' **own** corpus silhouette as control (`corpus:<Species>`) |
| **S2** | 2 | **silhouette, then texture** — the user's proposal, stage 1 produces the shape, stage 2 paints it |
| **S3** | 2 | probe → nearest bank body plan → render (`--control auto`, implemented, unmeasured) |
| **S4** | 2 | render, then a low-denoise **refine** pass to clean outline and flats |
| **S5** | N | **candidates + gate + retry** — batch, score by `iou_ref`, keep the best |

Other avenues carried in [forks](forks.md) rather than dropped: a hires second pass at higher
resolution, palette conditioning from the real sprite via IP-Adapter, per-stage knob splits (the
strength that suits a silhouette stage is not the one that suits a texture stage), and the
edit-model restyle once Qwen-Image-Edit finishes downloading.

## Design stance

- **Ground truth decides.** `iou_ref` against the real sprite is the primary number; a method that
  obeys its control while missing the animal must score badly, and this is the metric that can see
  that.
- **The eye still ratifies.** Carried from
  [beat-e07 F3](../2026-08-02-lora-beat-e07/forks.md#f3) — five times now the images have overturned
  the numbers, most recently on 2026-08-08 when the metric called two prompt forms equivalent and
  one of them had a glowing disc stamped on the wolf's back.
- **One direction.** East is the strongest cell in every run this project has done; if a method
  cannot win there it cannot win anywhere. Other directions are follow-up work, by instruction.
- **Count the stages honestly.** More stages cost wall-clock and add failure modes. A stage earns
  its place by moving `iou_ref`, not by being architecturally tidy.
- **Do not re-open the LoRA.** `r20g07` is frozen input, as in the predecessor.

## Future intent this plan must not trim

- **Untrained species are the real product** and they are in scope here ([P4](todo.md)), after the
  method is settled. The user expects them to need "extra or different steps" — the plan must not
  quietly declare victory on trained species and stop.
- **The other directions are a follow-up stream, not a deletion.** Whatever wins here must be
  expressible for south and north; a method that only works on side profile is a partial answer and
  must be recorded as one.
- **The corpus template library is the sprite-gen plan's "reference library"** in a different guise.
  If S1/S2 wins, that plan's phase 1 is effectively done and it should be said so there.
- **Hue is discarded downstream** — the game re-tints via layer maps and only brightness survives.
  Colour fidelity still matters for the layer-map decomposition, which needs findable material
  regions.
