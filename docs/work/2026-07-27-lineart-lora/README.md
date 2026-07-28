# Line-art LoRA — train on decoloured sprites so markings become geometry — 2026-07-27

_Component: [`dev/art`](../../components/dev/) · `bin/lib/{decolour_corpus,split_layers,build_quad,prep_train}.py`
+ a new LoRA. Plan in [`todo.md`](todo.md); decisions in [`forks.md`](forks.md); findings in
[`issues.md`](issues.md). Follows [`2026-07-26-sprite-eval-trust`](../2026-07-26-sprite-eval-trust/README.md)
(done 18/18). Shipping LoRA remains `rd_quadruped_e07`._

## The user's idea, and why it addresses a measured ceiling

**Train a LoRA on colour-free sprites**, so what it learns is *structure* — outlines, and the
markings the source art draws as filled black shapes (tiger stripes, jaguar spots).

This targets a ceiling we measured rather than assumed. Marking **structure** proved immovable
through every inference-side lever:

| lever tried | effect on marking structure |
|---|---|
| LoRA strength (0.6–1.0) | none |
| CFG (4–8) | 24% metric spread, **visually imperceptible** |
| prompt ("cross fox", "banded tail") | changes **tone**, not structure |

All three produced 0.11–0.14 marking variation — the same band. The diagnosis: the corpus holds
**one fox**, so the LoRA learned one fox's light/dark layout and re-tones it. Colour is the wrong
axis anyway, since the game re-tints downstream via the layer maps.

## Why decolouring is the right transform (not thresholding)

The first attempts — luminance thresholds, edge detection — could not separate *black line work*
from *dark fur*: a wolf at `<110` came out 74% black. Both were **my** detours; the user's proposal
was to run the existing layer pipeline and reconstruct with a flat tint:

```
out = residual + Σ(layers.ch × TINT)
```

The residual already holds the black line work; the per-pixel layer coefficients already hold
brightness. So a uniform tint removes **only hue** — which the earlier round-trip analysis showed is
the *only* component the layer split loses anyway (brightness survives to ~1/255, and the entire
14.3/255 mid-tone error is perpendicular hue drift).

**TINT = 200 grey**, chosen by measurement: it keeps 78% of the tonal range while leaving a 55-level
gap to the white plate. At 255 the palest subject (bear) lands exactly on the background and
disappears; at 128 a dark coat crowds against its own black outline.

## Design stance

- **Reuse the working prep unchanged.** ESRGAN upscale + deterministic jittered fill at 1024 —
  the same path that built the run-4/run-5 datasets. Only the *pixels* change.
- **One acceptance test decides whether this generalises**: ask the trained LoRA for an **anteater**
  line drawing. The anteater has no body-plan analogue in the corpus (`--control auto` declines it at
  0.53). If the LoRA invents a plausible long-snouted form, it beats silhouette lookup; if it returns
  a boar, the ceiling is the corpus either way and lookup was already sufficient.
- **Say plainly if it loses.** Runs 4 and 5 both failed to beat `e07`. This is a simpler learning
  target, which is a point in its favour, but that is a hope and not a prediction.

## Future intent this plan must not trim

The output is meant to feed the **two-stage architecture** the user described: generate a line
drawing → use it as the ControlNet source → render detail into it with a separate pass. That makes
the LoRA a *silhouette/structure generator*, replacing the 132-entry bank lookup with something that
can invent shapes. Keep it body-plan agnostic — the biped/legless/humanoid LoRAs would each want the
same treatment.
