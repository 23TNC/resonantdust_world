# Forks — decisions taken, with what was rejected

## F1 — Decolour via the layer pipeline, not by thresholding · RESOLVED 2026-07-27

**Reconstruct with a flat tint** (`residual + Σ layers×TINT`), per the user's proposal.

Rejected: **luminance thresholding**. It cannot distinguish black line work from dark fur — at
`<110` the wolf came out 74% black and the fox 62%, both solid masses rather than drawings. Tightening
to `<60` fixed the canines but silently dropped the jaguar's mid-grey spots; no single threshold
serves both, because the source draws markings at different darknesses.

Rejected: **edge detection**. Uniform across coat darkness, but it traces markings as *outlines*
rather than keeping them filled, so a tiger's stripes become hollow contours — losing the very
structure this LoRA is meant to learn.

The layer route avoids both because the split already separates line work (residual) from material
regions (layers); only hue is discarded, and hue is the one component the round-trip loses anyway.

## F2 — Tint = 200 grey · RESOLVED 2026-07-27

Measured on the bear, the palest subject, against a 255 white plate:

| tint | subject range | gap to plate |
|---|---|---|
| 128 | 0–128 | 127 |
| 160 | 0–160 | 95 |
| **200** | **0–200** | **55** |
| 255 | 0–255 | **0** |

**255 is disqualified**: the bear's brightest pixel lands exactly on the background, so a white
animal on a white plate has no interior signal at all. **128 wastes half the range**, crowding dark
coats against their own black outlines. 200 keeps 78% of the range with a comfortable margin.

Tied to the white plate — if the background ever changes, this choice moves with it.
