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

## 2026-07-25 — P2 complete (the corpus IS the template library)

**`bin/lib/silhouette_bank.py`** builds `.staging/silhouette-bank/<Species>/<dir>.png` from the
training corpus: **132 species × 3 directions = 396 control images**, zero art authored. No species
came out incomplete. It stores the *sprite on white*, not a pre-baked edge map, so
`generate.edge_map()` stays the single place edges are derived — verified byte-identical for
Wolf_Timber / Bear / Tiger (`--verify` → PARITY OK), which also keeps `--edge-thresh` live.

**Family table**: all **11** families from `build_quad.py` resolve to a representative chosen for a
clean, characteristic silhouette. An unknown family errors with the valid list
(`unknown family 'sasquatch'. valid: bear, camel, canine, cattle, deer, equine, feline, pachyderm,
pig, primate, rodent`).

**The acceptance test — [I2](issues.md#i2)'s bear, three control modes, same species:**

| control | east | south | north |
|---|---|---|---|
| `template` (wolf art) | 8.1% PASS | 46.3% PASS | 47.8% PASS |
| `none` (txt2img) | 5.7% PASS | **136.6% FAIL** | 36.0% PASS |
| **`family:bear`** | **1.3% PASS** | **4.1% PASS** | **1.2% PASS** |

(d_aspect vs the real Bear sprite.) The corpus silhouette fixes **both** prior failures at once: the
cross-family homogenisation of the templated path *and* the facing collapse of the template-free
path. Visually (`.staging/p2-bear/compare.png`) the `family:bear` row matches the real corpus row —
proper bear side profile, a front view with the bear's own body shape instead of a wolf tail with a
face inside it, and a rear view with round ears.

**Colour is not constrained by the bank** — checked because the first bear came out cream from a
near-white (238,238,238) control image. Prompting "black bear, jet black fur" produced mean subject
RGB (49,57,59); "brown bear" gave (193,190,177). So the control image supplies **shape only**, as
intended at `dn=1.0` where its latent is destroyed. The cream result was prompt/seed variance, not
bleed.

## 2026-07-25 — P3 complete (generate-many + auto-screen)

**`--candidates N`** wraps the direction loop, so seeds `seed..seed+N-1` each produce their own
variant leaf with its own IP-anchor chain (the east hero is re-derived per candidate, not shared —
sharing it would couple the candidates and suppress exactly the variety we are sampling for).

**`--ref <Folder>[:<stem>]`** names the real corpus sprite that defines correct proportions.
`score_sprite()` runs the [F4](forks.md#f4) gate at write time. Without `--ref` the aspect check is
*skipped* rather than defaulted — inventing a reference silently would be worse than screening on
structure alone.

**Verified on a 4-candidate bear batch** (`--control family:bear --ref Bear --candidates 4`):
4/4 passed, `scores.csv` written with one row per sprite (12 columns incl. blobs / bg / bg_uni /
fill / aspect / solidity / d_aspect), and the summary named the winner —
`best e: seed 8901 d_aspect=0.1%`, `best s: seed 8901 d_aspect=4.5%`.

**Quarantine verified deterministically.** A live batch happened to pass 3/3, which does not
exercise the reject path, so `report_candidates()` was driven directly with a synthetic failing
leaf: the passing leaf stayed at `100/`, the failing one moved whole to `_rejected/101/`, and the
summary reported `1/2 candidate(s) passed the gate; quarantined [101]`. Rejection is per leaf, not
per sprite ([F5](forks.md#f5)).

**Finding while testing ([I6](issues.md#i6)):** the south-view failure is *seed-dependent*. The same
template-free bear-south that scored 136.6% (FAIL) at the P0 seed scored 43.5 / 46.3 / 14.9% (all
PASS) at three others. P0's single-seed matrix measures a failure *rate*, not a species verdict —
and that stochasticity is precisely what makes P3's sample-and-screen the right answer.

## 2026-07-25 — P4 complete (e/s/n coherence without any authored art)

**Coherence measured** as Bhattacharyya distance between per-direction RGB histograms over *opaque
pixels only*, on a red fox generated at one seed in both modes. The scale comes from the real
corpus, so "coherent" means "as coherent as our own art":

| | mean cross-direction distance |
|---|---|
| REAL corpus, same animal (Wolf/Bear/Fox e vs s) | **0.028** |
| our generated set, `family:canine` | 0.169 |
| our generated set, `control none` | 0.193 |
| REAL corpus, *different* animals (Wolf vs Tiger, Bear vs Fox) | **0.565** |

**Call: PASS on identity, but honestly loose.** Both modes land far closer to same-animal (0.028)
than to different-animal (0.565), so the IP anchor *is* carrying identity across directions even
when east was generated rather than drawn — the sets read as one fox. But at ~6× the real-art
distance they are measurably less consistent than hand-authored sets, so this is not parity.

**A measurement error caught mid-way:** the first reference scale was computed against
`.staging/silhouette-bank/` images, which are RGB-on-white with **no alpha** — so the "opaque only"
mask selected the whole frame and the white plate dominated every histogram, deflating
different-animal distance to a meaningless 0.078. Recomputed against `.staging/animal-lora/` sprites,
which carry real alpha. The corrected different-animal mean is 0.565.

**[F3](forks.md#f3) resolved to per-direction corpus silhouettes**, which is what `--control
family:` already does. Geometry is the deciding factor, not colour: template-free south and north
collapsed to **head-only busts** on the fox (`.staging/p4/fox.png` row 1) while `family:canine` gave
full-body front and rear views with the correct tail-up convention (row 2). The IP-adapter carries
appearance, not shape — so it cannot fix a missing body.

**Acceptance met:** `--from pawn/animal/_p4 --to _p4-fam --control family:canine --dirs e,s,n`
produced a coherent three-direction fox with **no hand-authored art at any step** — the kind has no
template, and the control silhouettes came from the corpus bank.

## 2026-07-25 — P5 complete (validated on species the LoRA has never seen)

Five species confirmed **absent from `.staging/quad-lora-train`** (0 corpus files each) generated at
e/s/n through `--control family:<f>` + `--ref`, borrowing a body-plan relative's silhouette:

| species | family rep | gate | notes |
|---|---|---|---|
| meerkat | rodent (Capybara) | 3/3 | tan, dark eye patches — but lying, not upright |
| armadillo | rodent (Capybara) | 3/3 | banded armour plates clearly rendered |
| okapi | deer (Deer) | 3/3 | white-striped legs on the rear view |
| wombat | bear (Bear) | 3/3 | weakest identity — reads as a generic stocky mammal |
| aardvark | pig (Pig) | 3/3 | long snout, large ears |

**15/15 valid (100%)**, best-east d_aspect 2.6–14.6%. `.staging/p5/unseen.png`.

**Limits recorded in [I7](issues.md#i7):** the constraint is *pose*, not species — the generator can
only produce a stance some corpus silhouette already holds. The meerkat's upright sentry stance has
no representative in a quadruped corpus, so it cannot be asked for. Species the LoRA never trained on
are otherwise handled well.

---

### Stream complete — 22/22

The pipeline that could only reproduce species it already had art for now generates arbitrary
quadrupeds with **no hand-authored control art**, screens its own output, and was validated on
species outside its training data.
