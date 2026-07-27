# Completed — verification log

_Append-only. Dated entries: what landed and **how it was checked**. Item text lives in
[`todo.md`](todo.md) with its box ticked._

_(nothing landed yet — the stream opened 2026-07-26 with the plan only)_

## 2026-07-26 — P0 complete (the ruler now sees the pose)

**`iou_ref`** — IoU of the generated silhouette against the real corpus sprite, both cropped to their
own bbox and rescaled into a common grid first (normalising position and scale, so the metric asks
"is this the same *shape*" rather than "is it in the same place").

**It catches both run-4 failures that the gate missed:**

| | e07 | run-4 | result |
|---|---|---|---|
| **south** (framed busts) | 0.792 / 0.903 / 0.801 / 0.808 / 0.745 / 0.816 | 0.691 / 0.824 / 0.752 / 0.656 / 0.741 / 0.815 | e07 higher **6/6** |
| **east** (pose drift) | mean **0.694** | mean **0.619** | e07 higher **6/6** |

East is the important one: `d_aspect` scored it a **tie** (34.8% vs 34.1%) because it takes an
absolute difference. `iou_ref` separates it cleanly on every species.

**`d_aspect_signed`** confirms the mechanism directly: e07 east **+27.9%** (14/18 positive — too
long, a proportion wobble) versus run-4 **−29.7%** (only 3/18 positive — too compact, i.e. sitting
rather than lying). The sign was the whole signal the unsigned metric discarded.

**Gate calibration: no change adopted ([F3](forks.md#f3)).** Six variants scored against the 67
labelled sprites; the current gate's 3 errors beat every alternative, and adding `iou_ref` cost
false negatives (5–7 errors). The resolution is that two questions were being conflated — *is this
sprite usable* (gate) versus *is this model better* (`iou_ref`). `iou_ref` is adopted as the **A/B
ranking metric**, not a pass/fail check.

This is the second time a metric with real discriminative power turned out to be wrong as a hard
gate; the first was `d_fill` in the predecessor.

## 2026-07-26 — P1 complete (the visual check is part of the harness now)

`lora_eval.contact_sheet(cells, out)` tiles labelled images into one reviewable PNG — column headers
from the config, rows from the subject, blank cells for anything missing so a long run never aborts
on one bad file.

**Wired in with no extra flag:**
- `lora_eval --pipeline` writes `sheet.png` beside `sweep.csv`, one column per dn/cn/cn_end config
- `generate.py --candidates` writes `sheet.png` into the output kind, **with the gate verdict in the
  column header** (`e ok`, `s REJECT`)

Both print the sheet path with a line telling the reader to look at it *before* trusting the table.

**Verified:** a 2-config × 6-species call produced a readable 409×1243 grid; a live 3-candidate
`generate` run wrote a 115 KB sheet showing all 6 sprites with their verdicts.

The sheet immediately earned itself — the e07-vs-run4 east grid makes the pose drift obvious at a
glance (e07 lying low, run-4 sitting up with a raised head, curled cat, upright fox) and also shows a
**white frame around run-4's tiger on EAST**, i.e. the [I3](issues.md#i3) frame artefact is not
confined to south as the earlier analysis assumed.
