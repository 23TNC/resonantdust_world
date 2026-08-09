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
