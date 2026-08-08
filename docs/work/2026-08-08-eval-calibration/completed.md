# Completed — eval calibration

_Dated entries: what landed and **how it was verified**. Newest last._

## P0 — The labelled set and the agreement check

- **2026-08-08 · P0.1 · `art eval-data` builds the labelled set from REAL pipeline output.**
  Generates into `.staging/eval_data/new/` for hand sorting; each seed is one pipeline call covering
  e/s/n so south and north anchor on the east hero the way the shipping pipeline does. Random
  32-bit seeds. Filenames carry species, direction, method, LoRA, generation resolution and seed, so
  provenance survives being moved by hand.

  **It never deletes, never overwrites, and scans EVERY folder under `eval_data/` before choosing
  seeds** — not a hardcoded `good`/`bad`, which was the first version and could not see the user's
  own `0-e`/`reject-e`/`good-s` taxonomy. That version reported "0 seeds already taken" against 10
  sorted files and would have re-offered work already judged.

  Two corrections the user had to make, recorded because both were me overriding their intent:
  I **restored six files they had deliberately deleted** while triaging, and the tool **created
  `good/` and `bad/` folders** it had no business creating. Both undone.

- **2026-08-08 · P0.2–P0.4 · `art eval-agreement` scores the labels — and `iou_ref` fails.**
  63 hand-sorted wolf sprites. Every metric we compute, per direction, against the majority-class
  baseline, **leave-one-out**:

  | | n | baseline | `iou_ref` | `d_aspect` | best |
  |---|---|---|---|---|---|
  | east | 31 | 84% | **84%** | 90% | `d_aspect` 90% |
  | south | 19 | 74% | **63%** | 63% | `sat` 95% (suspect) |
  | north | 13 | 62% | **38%** | 46% | `fill` 77% |

  **`iou_ref` never beats guessing**, and on north it is anti-correlated. The mechanism was written
  down before it was measured — [east-pipeline F4](../2026-08-08-east-pipeline/forks.md#f4) says a
  correctly-shaped sprite painted wrong "scores perfectly" — and south is exactly that case: the
  control guarantees the silhouette, the face breaks, and a silhouette statistic cannot see a face.

  **Verified by holding out.** Fitted in place, `iou_ref` looked like 87/79/69 and "beat baseline"
  everywhere. The difference between those two columns is the whole finding
  ([F2](forks.md#f2)).

  **What it does not retract:** the east-pipeline method rankings. `iou_ref` compared methods in
  aggregate across 18 cells, where wrong-animal silhouettes are exactly what it can see. Aggregate
  method comparison and per-image selection are different jobs; one statistic does not do both.

- **2026-08-08 · Resolution ([I4](issues.md#i4)) · the control image sets the generation size, and
  it has been 512.** The i2i latent is a `VAEEncode` of the control, the bank is 512×512, and SDXL
  is native at 1024. Within that frame the directions are not comparable — the east subject spans
  510 of 512 px, the **south subject spans 202** — so a frontal face is drawn in ~100px. Generating
  at **1024** turns the splayed white mask into a wolf face on the same three seeds.

  A hypothesis I checked first and disproved: that south's control carried more interior edge for
  ControlNet to trace. Its interior-edge fraction is **20%, identical to east's**.

  `--frame-fill` was implemented, measured, and left **off** — it does not fix the face and pulls the
  source plate's border into frame.

  **And the number that matters is still untested: the LoRA was trained at 768**, verified in
  `train_run20.sh` and the `_0768x0768_sdxl.npz` cache stamps. LoRA 768, base 1024, pipeline 512 —
  three different resolutions, and the only one the LoRA has seen is the one nobody has run.
  [P2](todo.md).
