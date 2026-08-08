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
