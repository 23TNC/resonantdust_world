# Completed — north/south evaluation

_Dated entries: what landed and **how it was verified**. Newest last._

## P0 — Labels worth fitting to

- **2026-08-08 · P0.1 · Generation resolution settled at 768 — the LoRA's own
  ([F5](forks.md#f5)).** Same three seeds, wolf south, `corpus:Wolf_Timber`, everything else fixed.
  **512** gave splayed white masks on all three (the failure the user rejected 12/12). **768** gave a
  proper wolf face on all three — eyes, muzzle, dark nose — and stayed closest to the corpus
  proportion. **1024** also resolved the face but drifted busier: spikier fur, more interior strokes,
  heads growing against the body.

  **This corrects my own earlier recommendation of 1024**, which I had argued from "SDXL is native
  there". Incomplete: the LoRA has only ever seen 768, and it is the LoRA that carries the
  convention. At 1024 the base's detail prior has room to add strokes the corpus does not have. The
  user's question — *"did we use the 1024 scaled-up dataset?"* — is what sent me to
  `train_run20.sh` (`--resolution="768,768"`, caches stamped `_0768x0768_sdxl.npz`). Without it the
  stream would have adopted 1024 on a half-argument. 768 is also ~2.25× the compute of 512 against
  1024's ~4×. `art eval-data` now defaults to it. Sheet: `.staging/p0-resolution.png`.

- **2026-08-08 · P0.2/P0.3 · `art eval-sort` records WHY, and separates `borderline` from `bad`.**
  Buckets stay `<label>-<dir>` so every existing reader keeps working; the reason rides in a sidecar
  `labels.json` keyed by filename rather than in the name, because files get moved by hand and a name
  would then have to be rewritten. Reasons: `background`/`lineart`/`anatomy`/`pose`/`colour`/`other`,
  and a reject without one is refused rather than silently accepted. `--list` surfaces any reject
  missing a reason, since those are rows [P3](todo.md) cannot use.

  **Verified by backfilling the three the user explained**, in their own words: `4110` → `bad`
  /`background` (the unkeyed plate), and `4108` and `781057776` → **`borderline`**/`lineart`, both
  *"if pressed I'd take it as acceptable"*.

  That reclassification alone changes the east scoreline it is scored against: what read as **3
  metric errors is 1 clear miss and 2 boundary calls**, and a judge that agreed with the user's
  hesitation was being marked wrong for it.

## P1 — The vision QA gate

- **2026-08-08 · P1.1/P1.2/P1.4 · The gate is built and scored. It works on EAST and does not yet
  work on north or south.** `art eval-gate` sends one sprite — composited on a **checker**, so an
  unkeyed white plate is visible rather than invisible — and returns structured JSON on the design
  doc's four questions plus the two failure modes the user actually rejected for.

  | | n | baseline | v1 | v2 | v3 |
  |---|---|---|---|---|---|
  | east | 31 | 84% | 19% | **97%** | **94%** |
  | south | 19 | 74% | 79% | 47% | 58% |
  | north | 13 | 62% | 62% | 62% | 62% |

  **4.1 s and ~1 call per image**; the full 63 took ~4 minutes. That is cheap enough for 12
  generations × 6 species (216 images ≈ 15 min), so cost is not the obstacle.

  **First run, east 19% — my prompt, not the model.** The gate rejected **25 of 26** east sprites the
  user passed, every one for *"no legs visible; body reads as legless blob"*. I had written "limbs
  resolve as real features" into the system prompt, importing a naturalistic anatomy standard that is
  backwards here: the RimWorld convention is a **solid mass with an unbroken bottom edge**, and the
  user's own correction on run-9 was *"We do not want legs on our animals."* Stating the convention
  took east from 19% to 97%.

  **Second run, south 79% → 47% — the same failure in a new costume.** Describing the south
  convention gave the gate a *structural checklist*, and it started passing broken sprites with the
  reasoning *"Tail spike top, tapered body, small face bottom, matches convention."* That structure is
  imposed by the ControlNet, so checking it is exactly the mistake the geometric metrics make
  ([I1](issues.md#i1)). v3 made the face check explicit and primary, recovering 47% → 58% — still
  below the 74% baseline.

  **North is unusable and the number hides it: the gate called 13 of 13 GOOD.** Its 62% is the
  majority-class baseline reached by never rejecting anything, which is not agreement.

- **2026-08-08 · P1 · Stopped tuning, and why.** Three prompt revisions scored against the same 63
  images is **fitting in place** — the methodological error [F2](forks.md#f2) exists to prevent,
  wearing different clothes. South swung 79 → 47 → 58 across those three, which on n=19 is a signal
  that the number is unstable rather than that the third prompt is best.

  Compounding it: **the 19 south labels were all generated at 512**, and [F5](forks.md#f5) has since
  moved generation to 768 — so the gate is being fitted to a defect distribution we have deliberately
  replaced. Continuing to tune against them would optimise for splayed-mask faces that the new
  resolution largely removes.

  **Verdict, honestly:** the gate is **usable on east today** (94–97% against an 84% baseline, and it
  independently caught the unkeyed background with `keyed_background: false`). North and south are
  **not answered**, and the blocker is data, not prompt engineering.
