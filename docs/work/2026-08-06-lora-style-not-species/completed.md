# Completed — style, not bestiary

_The verification log: dated entries saying what landed and **how it was checked**. Append-only._

_Nothing delivered yet. Items land here with their measured result when ticked in
[`todo.md`](todo.md)._

## P0 — Measure the corpus before changing it

- **2026-08-06 · P0.1 · The claim is confirmed exactly.** 200 species / 701 images / mean 3.50.
  **176 of 200 (88%) have exactly 3 images**, and **176 of those 176 have exactly one per
  direction** — not approximately, exactly. Direction totals are balanced: east 234, north 233,
  south 234. The corpus is thin, not skewed.
- **2026-08-06 · P0.2 · 252 distinct non-`rd_` caption tokens, and 229 of them (91%) appear on ≤3
  images** ([I2](issues.md#i2)). The vocabulary splits cleanly: a dozen direction/composition
  phrases carry 233–234 examples each, everything else is a species or family name with ≤3.
- **2026-08-06 · P0.3 · A caption recorded verbatim** so the diff to the style-only form is legible:
  `rd_style, rd_animal, rd_quadruped, rd_east, side profile, side view, facing right, feline,
  tiger, single creature, full body`. Nine of eleven tokens have 233+ examples; `feline` and
  `tiger` have three.

## P1 — Build the style-only caption variant

- **2026-08-06 · P1.1 · The rule is a WHITELIST, not a species blacklist** ([F1 addendum](forks.md)).
  Keep every `rd_*` tag plus twelve direction/composition phrases; drop everything else. A blacklist
  would need 229 names enumerated and silently keeps anything it misses — a missed species token is
  exactly what this stream exists to remove. A whitelist fails safe.
- **2026-08-06 · P1.2 · 701 style-only captions written** to `.staging/animal-lora-styleonly/`,
  same stems, **originals intact at 701**. 243 distinct tokens dropped. Image files untouched.
- **2026-08-06 · P1.3 · Ten diffed by eye** — every drop is a family or species name and nothing
  else: `cattle, muffalo` · `primate, orangutan` · `bird, quail female` · `deer, toxdeer female` ·
  `rodent, beaver` · `bear, bear`. Convention, body-plan and direction tokens intact throughout.
- **2026-08-06 · P1.4 · No empty captions; all six body-plan tags still discriminate.** 0 empty of
  701. quadruped 498/701, biped 90, winged 69, humanoid 51, legless 27, multiped 27 — every one has
  negatives, so [I12](../2026-08-02-lora-beat-e07/issues.md#i12)'s constant-tag defect is not
  reintroduced. **27 distinct captions remain and 165 images share the largest**; recorded as
  [I3](issues.md#i3) with an explicit prediction before training rather than a rationalisation after.

## P2 — Train it

- **2026-08-06 · P2.2 · Caption source verified at launch, and a failed run proved it twice.**
  The log names `dataset_styleonly/1_animal/...` as the data path and `read caption: 701/701`
  completed — so kohya read the NEW captions, not the originals. `load network weights` count is
  **0**, confirming a fresh single-stage run with no warm start.
- **2026-08-06 · P2 · First launch FAILED on permissions** ([I4](issues.md#i4)), not on anything to
  do with this stream's thesis. `--cache_latents_to_disk` writes a `.npz` beside each image, and I
  had built the dataset over SSH as root (`0:0`) while the working one is `1025:1025`. Fixed by
  `chown --reference`; relaunched and caching now proceeds past the file that failed.
