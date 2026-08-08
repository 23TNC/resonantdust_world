# Completed — direction consistency

_Dated entries: what landed and **how it was verified**. Newest last._

## P0 — A ruler before a change

- **2026-08-08 · P0.1/P0.2 · `generate.py` now measures its own output on every run.**
  Added `interior_metrics()` (luminance / saturation / coverage over **opaque pixels only**) and
  `report_consistency()` (per-direction lines + the cross-direction luminance **spread**), wired
  into the generation loop and defaulting **on**, with `--no-metrics` to suppress.

  **Opaque pixels only, and no shape term, on purpose.** At `cn ≥ 0.5` the silhouette is the
  authored template, so anything measured on shape is measuring the templates
  ([I4](issues.md#i4)). Only the interior is evidence about this stream's changes.

  **Verified two ways.** Synthetically, against flat plates of known value — 0/128/255 return
  lum 0.0/128.0/255.0 at cov 25.0%, a fully transparent sprite returns `None` rather than a zero
  row, and feeding the three hand-measured wolf figures back through `report_consistency`
  reproduces **spread=24.9, 2.8x over** exactly. Then end to end on a real run:

  ```
  seed 9200  e: lum= 118.4 sat= 43.5 cov= 28.4%
  seed 9200  s: lum= 115.7 sat= 33.8 cov= 17.2%
  seed 9200  n: lum= 130.1 sat= 33.5 cov= 20.0%  <- outside corpus band 62-121
  seed 9200  spread=14.4  (corpus ceiling 9.0 — 1.6x over)
  ```

  **Read of the item, recorded because it is a judgement call:** the item says "add a `--metrics`
  flag" *and* "one line per direction on **every run**". Those pull opposite ways, so it is
  `--metrics` defaulting on with `--no-metrics` to opt out — a flag exists, and the default is
  that no sheet is judged by eye alone.

  The spread is computed **per candidate seed**, not pooled: a variant leaf is what ships as a set,
  so pooling seeds would report a number no single sprite set actually has. One direction prints
  `n/a (nothing to be consistent with)` rather than a misleading 0.0.

- **2026-08-08 · P0.3 · The consistency set is pinned and the harness reads it.**
  `bin/lib/consistency_set.json` (override via `RD_CONSISTENCY_SET`) pins subjects, seeds,
  directions, LoRA, `cn`/`cn_end`/strength and both thresholds; `bin/lib/consistency_eval.py` runs
  it and writes `results.json`. Mirrors what `eval_set.json` does for the LoRA harness.

  **Four subjects, each answering a different question** so a change that helps one and breaks
  another stays visible: **wolf** (the incumbent, the only baseline we have), **bear** (dark-end
  corpus value — where the too-pale gap should hurt most), **fox** (the corpus's most saturated
  animal at sat 70.2 against a grey wolf's 10.6–17.5), **anteater** (no corpus analogue — the
  generalisation check carried over from the predecessor). Three seeds, because the predecessor
  measured two identical training runs differing by 17.31/255 and the six-seed sweeps showed seed
  dominates north/south — a one-seed verdict is noise.

  **A limit is recorded in the file rather than hidden:** every subject reads the **wolf** template
  triple, because only quadruped templates exist and swapping templates per subject would change
  the silhouette between cells and make the interior numbers incomparable. So the set measures
  *consistency*, not species fidelity — the bear and anteater come out wolf-shaped, which is fine
  for the question being asked.

  **Verified, including a silent-fallback check that mattered.** The harness invokes `generate.py`
  once per direction, so the east "hero" that anchors south and north has to survive a *process
  boundary*. It does — a `--dir s` run on an existing leaf prints `IP anchor: existing
  9101/sprite.e.0.png`. Had that fallen back silently, the whole baseline would have measured a
  pipeline with no identity anchoring at all, and read as a much worse number for the wrong reason.
  Smoke run on wolf/9101 returned `e=158 s=139 n=126 spread=31.2 OVER`.

- **2026-08-08 · P0.4 · Baseline recorded. The single wolf measurement was representative, and it
  turned up a mechanism I had diagnosed backwards.** 36 generations, 4 subjects × 3 seeds × 3
  directions, `r20g07` at `cn 0.5 / cn-end 0.9 / strength 0.7`, trained-tag prompt form.

  | subject | mean spread | worst | clears 9.0 |
  |---|---|---|---|
  | wolf | 20.1 | 31.2 | 1/3 |
  | bear | 33.7 | 61.0 | 0/3 |
  | fox | 22.3 | 37.8 | 1/3 |
  | anteater | 17.6 | 28.9 | 0/3 |
  | **all** | **23.4** | **61.0** | **2/12** |

  Today's single wolf figure of 24.9 is **confirmed, not corrected** — the population mean is 23.4.
  The bear is the worst subject at 33.7, exactly as predicted when it was chosen for sitting at the
  dark end of the corpus, so that subject earned its place rather than padding the set.

  **Absolute value is the larger defect and it is systemic:** only **4 of 36** sprites land inside
  the corpus band of 62–121, mean 143. A pipeline whose views agreed perfectly would still be
  uniformly too pale, which is why [F4](forks.md#f4) tracks the two separately.

  **Saturation does NOT collapse universally — my earlier claim was wolf-only.** Across the set,
  saturation runs 3–79, mean 30. The grey wolf measures 4–10 (genuinely flat), but bear and fox run
  20–79, which is in the corpus's own range. "Saturation collapsed to 4–6" was measured on one grey
  animal and does not generalise; the defect is species-dependent.

  **The finding that redirects the stream ([I6](issues.md#i6)): east is the outlier, not south and
  north.** Decomposing each set's spread — east is the extreme in **11 of 12** sets, mean
  `|east − mean(south,north)|` is **19.3** against mean `|south − north|` of **7.7**, a 2.51×
  ratio, and south-versus-north already clears the 9.0 ceiling in **8 of 12** sets. East is the one
  direction rendered by a different graph (`graph_hero`, no IP-Adapter) while south and north both
  go through `graph_ip`. So the IP-Adapter is carrying value *well* between the views that have it,
  and the gap is between two graphs rather than among three drifting siblings. [I2](issues.md#i2)
  said the opposite; it was written from one wolf set and a code read, and three views cannot
  separate "s and n drift apart" from "e stands apart". [P2](todo.md) reordered accordingly.

  **A harness defect found by the acceptance count ([I7](issues.md#i7)).** The baseline returned 35
  sprites for 36 cells: `report_candidates` quarantines a gate-rejected leaf into `_rejected/`
  *after* printing its path, so the harness measured `wolf/9103` as a two-direction set. Left
  unfixed it would bias every future label toward sets that happened to generate cleanly. Fixed
  with `resolve_written()` plus a `--measure-only` mode; re-measuring from disk recovered the cell
  (east lum 146.0) and completed all 12 sets. The aggregate did not move — that cell's east
  happened to fall between its own south and north — but the next draw would not have been so kind.

## P3 — The batched pass: a negative result, recorded before it cost GPU time

- **2026-08-08 · P3.1–P3.3 · The premise was wrong and the phase is closed without building it
  ([I9](issues.md#i9)).** The README called the batched sampler pass the stream's most promising
  capability, reasoning that three views denoised together share a noise schedule and a prompt
  evaluation. That holds only when batch items **share conditioning**, and ours cannot: each
  direction needs its own prompt *and* its own ControlNet template, while a `KSampler` broadcasts
  one conditioning and `ControlNetApplyAdvanced` one image across the whole latent batch. Three
  views with three prompts and three controls are three independent generations that happen to
  share a sampler call — throughput, not consistency.

  **Verified rather than assumed:** surveyed `/object_info` for the mechanism that *would* share
  information across a batch. There is no SDXL **`ReferenceOnlySimple`** on this box. What is there
  — `ReferenceLatent`, `USOStyleReference`, `FluxKontextMultiReferenceLatentMethod`, and a set of
  video-model reference nodes — belongs to architectures we are not running.

  **The corrected claim, now in the README:** 24 GB buys IP-Adapter *and* ControlNet *and* several
  reference images resident at once, which is [P2](todo.md). True reference attention is a property
  of the **edit-model** architecture ([P6](todo.md)), where `ReferenceLatent` is exactly how
  Qwen-Image-Edit conditions on a source image. The hardware claim was right; the mechanism was not.

  **Caught before any GPU time was spent**, by asking what the nodes can express before writing the
  graph. Same failure shape as [I5](issues.md#i5) — reasoning about a mechanism without first
  checking that the instrument can perform it — which is why it is recorded rather than quietly
  dropped.

- **2026-08-08 · P1.3 · The prompt form does NOT move the spread. It moves everything else.**
  Paired over all 12 cells, same seeds and knobs, prompt text the only difference:

  | | mean spread | worst | clears 9.0 | lum mean | in band | sat mean |
  |---|---|---|---|---|---|---|
  | **tags** | **23.4** | 61.0 | 2/12 | 143.3 | 4/36 | 29.7 |
  | prose | 21.5 | 65.1 | 2/12 | **131.4** | **10/36** | 57.5 |

  **Spread: −1.9, prose lower in 6 of 12.** Against per-cell swings of ±34 that is a coin flip, and
  both forms clear the ceiling in exactly 2 of 12. The item asked to "confirm it moves the spread or
  say it does not" — **it does not.**

  Prose *is* meaningfully closer on **absolute value** (131.4 against 143.3; 10 of 36 in the corpus
  band against 4). That is a real result and it does not make prose the winner, because the
  saturation figure that looks like an improvement is not colour — it is artifacts.

  **The eye overturns the metric, which is what [F4](forks.md#f4) exists for.** Sheet:
  `.staging/p13-tags-vs-prose.png`, wolf 9101. Prose's **north carries a glowing green-and-yellow
  disc** stamped in the middle of the animal's back, and its **south is truncated at the frame
  edge**. The tag row is a clean grey-and-white wolf in all three views with the correct upward tail
  spike. Prose's saturation of 57.5 is that hallucinated disc and its relatives, not pigment.

  **Kept: tags**, on the images, with the metric recorded as neutral rather than supportive. This is
  the fourth time in this project that the pictures have overruled the numbers.

  **Two measurement defects fixed on the way, both of which had made prose look better than it is.**
  First reading gave prose mean 17.9 against 23.4 — a large apparent win that was **survivorship**:
  three cells had gone missing, and the two recoverable ones turned out to be prose's *worst*
  (wolf 9101 at 65.1, anteater 9101 at 35.2). Causes were separate:

  - **[I7](issues.md#i7) again, at a later point.** The harness resolves each sprite's path right
    after writing it, but generate.py is invoked once *per direction* and each invocation
    quarantines its own leaf — so east could be written live and then moved into `_rejected/` by
    the later `--dir s` call. Resolving once at write time is not enough; `measure_cell` now
    re-resolves at the moment it opens the file. I had also guessed the missing sprites were
    keying away to nothing — checked it, and **0 of 34 were empty**, so that hypothesis was wrong
    and the stale path was the whole story.
  - **A transient HTTP failure** to ComfyUI killed two of bear/9103's directions. Regenerated;
    unrelated to the prompt form.

## P2 — Anchoring

- **2026-08-08 · P2.1 · East self-anchoring does NOT close the east gap. [I6](issues.md#i6)'s
  observation stands; its MECHANISM is falsified.** 48 generations (the extra east pass costs one
  per set).

  | | mean spread | worst | clears 9.0 | `e` is extreme | `\|e−mean(s,n)\|` | `\|s−n\|` | `\|s−n\|≤9` |
  |---|---|---|---|---|---|---|---|
  | baseline | 23.4 | 61.0 | 2/12 | 11/12 | 19.3 | 7.7 | 8/12 |
  | east-anchor | 22.3 | 54.7 | 2/12 | 10/12 | **19.6** | **4.9** | **10/12** |

  Mean moved −1.1 with 6 cells better and 6 worse — a coin flip. **The east gap is unchanged
  (19.3 → 19.6).** I6 proposed that east is the outlier *because* it alone skips the IP-Adapter;
  putting it through the same graph moved that number not at all, so the graph difference is **not
  the cause**. What did improve is `|s−n|`, 7.7 → 4.9, now clearing the ceiling in 10 of 12 — the
  adapter tightens the views it touches, which was never the problem.

  **A second hypothesis of mine, also tested and also wrong:** that the metric was confounded by
  silhouette coverage (east has the largest subject, so its dark outline is a smaller fraction of
  its area). Measured `corr(coverage, luminance) = +0.154` across all 36 sprites — too weak to
  explain a 19-point gap. The ruler is not the problem.

  **What the sheet shows, and it is not subtle drift.** `.staging/p21-east-anchor.png`, bear 9101:
  **east is a cream-bodied animal with a large orange tail while south and north are orange-brown**.
  These are different-coloured animals, not one animal with a luminance wobble. Colour identity is
  *content*, and `weight_type: "style transfer"` discards content by design — so
  [I3](issues.md#i3), not I6, is the live hypothesis, and [P2.2](todo.md) is the indicated next
  move rather than a speculative one.

- **2026-08-08 · P2 · PARKED on the user's redirect.** *"this bear looks like a wolf... and our
  rotational consistency is getting much worse. Lets take a step back."* Both observations are
  correct and the first is a defect I built: `consistency_set.json` points **every** subject at the
  **wolf** template, so a bear can only ever come out wolf-shaped. I recorded that as a deliberate
  limit when pinning the set — it makes the interior numbers comparable across cells — and it also
  makes species fidelity unmeasurable, which is the more important property. Successor stream:
  [`2026-08-08-east-pipeline`](../2026-08-08-east-pipeline/README.md).
