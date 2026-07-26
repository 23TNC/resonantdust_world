# Issues — the defects this stream attacks

## I1 — Training data is blurred and unnormalised (the biggest single lever)

Two independent defects in `bin/lib/prep_train.py`, both affecting **nearly every training image**:

**Blur.** The prep resizes with `Image.LANCZOS`. Most corpus sources are 256px (545 files), some
128 (72) and some 64 — upscaled to 768 that is a 3–12× enlargement, and LANCZOS *smooths* the result.
So the LoRA was taught soft boundaries on almost its entire dataset, which is a plausible cause of
the blobby bears, mushy legs and "pointy legs" complaints seen throughout.

The user's framing is the important one: **this art is effectively vector graphics rendered as
bitmaps** — flat regions bounded by hard outlines. That means the missing pixels are *reconstructible
from a correct prior* ("edges are crisp, regions are flat"), unlike a photo where an upscaler would
hallucinate. An edge-preserving / anime-line-art upscaler should therefore recover something close to
the original vector art, not invent detail.

**Scale.** Subject scale is never normalised. The bear nearly fills its frame; the cat occupies a
fraction of it. The LoRA correctly learned that scale is arbitrary, and generated scale drifts as a
result (observed: "bear huge, cat small in the same batch"). Normalising the alpha bbox to a fixed
frame fraction also *raises effective resolution* for small subjects, which compounds with the blur
fix rather than duplicating it.

## I2 — Control selection is a hand-written table, and it caused the anteater failure

`silhouette_bank.FAMILY_REP` maps 11 families to one representative each — all chosen by hand. The
giant anteater was routed to **Elephant** purely because the table says `pachyderm -> Elephant`.

Result: the east view is genuinely good (long tapered snout, hunched back, huge bushy tail, diagonal
shoulder stripe) but **south and north collapse into a cape-like blob** where the tail dominates the
silhouette and the body reads as a fan. The elephant silhouette gave it mass but not structure.

There are **396 silhouettes** in the bank and the selection consults none of them — it consults a
human's guess about taxonomy, when the thing that actually matters is *shape*. A similarity search is
both more likely to be right and removes a hand-tuned knob (README design stance).

## I3 — The gate scores proportions, not shapes: it has both error types

| | case | numbers |
|---|---|---|
| **False positive** | blobby anteater south / north **passed** | `d_aspect` 13.2% / 4.1% |
| **False negative** | good horned oryx **quarantined** | `solidity=0.349` vs a 0.35 threshold — failed by 0.001 |

Both follow from the same root: [F4](../2026-07-25-template-free-generation/forks.md#f4) measures the
**bounding box**, so a correctly-proportioned blob passes and a correctly-shaped animal whose horns
enclose empty bbox area fails.

Candidate fix — **IoU of the output silhouette against the control silhouette it was given**. The
pipeline already *has* the intended shape (it fed it to ControlNet), so it can ask "did the output
follow it?". That is much closer to a semantic check than any bbox statistic, and it costs nothing.

The oryx also shows the threshold itself is arbitrary at the boundary: 0.349 vs 0.35 is not a
quality difference. Whatever replaces it must be calibrated against hand-labelled art, not chosen.

## I4 — Cross-direction coherence is ~6× looser than hand-authored art

Bhattacharyya distance between per-direction RGB histograms (opaque pixels only):

| | mean |
|---|---|
| REAL corpus, same animal | **0.028** |
| generated, `family:` | 0.169 |
| generated, `control none` | 0.193 |
| REAL corpus, different animals | **0.565** |

Identity clearly transfers — 0.169 is far nearer same-animal than different-animal — but a set is
visibly less consistent than drawn art. Untested lever: the IP-adapter weight (`IP_WEIGHT = 0.6`),
never swept.
