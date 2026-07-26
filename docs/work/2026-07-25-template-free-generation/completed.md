# Completed — verification log

_Append-only. Dated entries: what landed and **how it was checked**. Item text lives in
[`todo.md`](todo.md) with its box ticked; this file records the evidence._

_(nothing landed yet — the stream opened 2026-07-25 with the plan only)_

## 2026-07-25 — P0 complete (baseline + gate)

**`--control none` in `lora_eval.py --pipeline`.** Verified by running
`--kind pawn/animal/_test-bear --control none --ref Bear`, a kind with **no template file on disk**
(`ls textures/pawn/animal/_test-bear/template.*` → nothing). It completed 2/2 valid with the gate
sourced from the corpus sprite (`gate=corpus:Bear aspect=2.30`), proving no template path is read.
dn/cn are collapsed to one config in this mode since neither has an effect without a control image.

**Baseline matrices.** `bin/lib/tf_baseline.py` (new) generates a fixed 10-species × e/s/n matrix in
either mode and scores every candidate against its **own** corpus reference sprite. Both runs landed
30 rows: `.staging/tf-baseline/scores-none.csv` (mean score 71.6) and `scores-template.csv` (78.8).
Templated is driven by the wolf art for every species, which is what exposes the cross-family bias.

**The headline measurement ([I4](issues.md#i4)):** template-free is valid **25/30 (83%)** vs templated
**29/30 (97%)** — but the deficit is *entirely* south (5/10 vs 9/10). East and north are **10/10 in
both modes**. Template-free south failures are a *facing* error: the model draws a side profile when
asked for a front view (aspect 1.36–2.02 vs references 0.28–0.84).

**Second finding, unplanned:** templated output is **6–11× less varied in proportion** than real
animals — generated aspect sd 0.055/0.044/0.012 (e/s/n) against the corpus's own 0.320/0.155/0.131.
Every species is pushed toward wolf proportions. This is [I2](issues.md#i2) quantified: the template
does not merely fail across families, it *homogenises* within them too.

**Gate set ([F4](forks.md#f4))** — `blobs==1 · bg_uni>=0.75 · d_aspect<=50% · solidity>=0.35`,
calibrated against six hand-judged sprites (rejects the wolf-east bust at 58.5% and the bear-east blob
at 56.3%; accepts the good tiger, the approved bear-north, and both wolf sprites at the new defaults).

**Harness defect found and fixed ([I5](issues.md#i5)):** the `bg` metric rejected a hand-approved
sprite for filling the frame. Added `bg_uni` (uniformity of non-subject border pixels) as the real
keyability test; `bg` retained for reporting.

## 2026-07-25 — P1 complete (the hard template requirement is gone)

**`--control` on `generate.py`**, defaulting to `template`. `load_template()` grew
`required=False` and returns `None` rather than `SystemExit`ing; `resolve_control()` dispatches the
mode. `corpus:`/`family:` deliberately raise a "not wired yet (P2)" error rather than silently
falling back — a silent fallback would hide a typo'd `--from`.

**`_tail()` builds two shapes now.** Verified by inspecting the submitted graph directly:

| | templated | `--control none` |
|---|---|---|
| nodes | `20,21,3,30,31,32,6,7,8,9` | `21,3,6,7,8,9` |
| latent | `VAEEncode` of the art | `EmptyLatentImage` |
| sampler positive | `["32",0]` (through ControlNet) | `["6",0]` (raw conditioning) |
| denoise | `DN` | forced `1.0` |

ControlNet nodes 30/31/32 are **absent**, not zero-strength — nothing is loaded or evaluated.

**End-to-end, on a kind with no template file on disk** (`textures/pawn/animal/_test-bear`,
`ls template.*` → nothing): `--control none --dn 0.5` printed
`control -> none (txt2img + LoRA; no ControlNet, no i2i latent)`, warned
`NOTE: --dn ignored under --control none … sampling at full strength`, and wrote
`7777/sprite.e.0.png`. Scored `blobs=1 bg_uni=1.00 aspect=1.51 (ref 2.30) d=34.4%` → **PASS** on the
[F4](forks.md#f4) gate. The pipeline can now generate a species it has no art for.

**Regression:** the default templated path is unchanged after the `_tail` refactor — wolf east at
the standard defaults scored `aspect=2.11 (ref 2.08) d=1.5%` → PASS.
