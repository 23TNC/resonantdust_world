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
- **2026-08-06 · P2.1 · Run-16 trained clean.** 2112/2112 steps in **1h 05m**, `TRAIN_EXIT_OK`,
  12 checkpoints, 60 samples, loss **0.0248 → 0.0194**. Config frozen; captions the only variable.
  It survived a container restart mid-flight (see [I5](issues.md#i5)) and completed unattended.
- **2026-08-06 · P2.3 · All 12 generations saved** as `rd_styl_anima_r16_g01…g12` in
  `models/loras`, **5.2 GB**.
- **2026-08-06 · P3.1 · Comparison sheet built** — run-16 vs run-10 interleaved at gens 8/10/12,
  same prompts and seeds, five subjects.

  **Result: the thesis holds on stability, and costs the thinnest body plan.**
  - **Run-16 is far more stable across generations.** Tiger, wolf, duck and bear are near-identical
    at g8/g10/g12. Run-10's wolf-south mutates every two generations — dark cap at g8, tongue-out
    face at g10, fragmented spiky bib at g12. Removing species tokens removed the churn.
  - **Run-16's wolf-south is the cleanest in the project** — symmetric, ears resolved, correctly
    proportioned, and *stable*. That is the subject that broke in every previous run.
  - **[I3](issues.md#i3)'s blurry-average prediction did NOT happen.** 165 images sharing one
    caption produced crisp silhouettes, not mush. Prediction recorded before the run; refuted by it.
  - **Cost: the snake.** Run-10 coils at g8; run-16 gives a rearing cobra at g8 then a small flat
    green disc at g10/g12. `rd_legless` has only 27 examples and losing its species words hurt it.
  - Tiger is smaller and flatter; the duck gains a belly fold at g10/g12. Both runs are white —
    **value is unchanged by captioning**, as expected, since it was never a caption problem.

  **What this does NOT yet show:** every subject here is one the corpus contains. The actual claim —
  that species identity now comes from the PROMPT — is untested until [P3.3/P3.4](todo.md).
- **2026-08-06 · P2.3 corrected · 12 of 12 saved, after catching a silent drop.** The first pass
  promoted only **11** — kohya names the final generation `rd_styl_anima.safetensors` with no
  number, so a `-0000*` glob loses it ([I6](issues.md#i6)). Caught by the acceptance count, not by
  eye. Total **5.2 GB**.
- **2026-08-06 · P3.3/P3.4 · THE THESIS IS CONFIRMED.** Same prompt, same seed (7700), same
  strength (0.85), r10 g8 (species captions) vs r16 g8 (style-only). Sheet:
  `.staging/p3-thesis-test.png`.

  | subject | corpus support | r10 species captions | r16 STYLE-only |
  |---|---|---|---|
  | **anteater** | **none** — no body-plan analogue | brown blob, dot for an eye, blunt snout | pale facial marking, **defined eye**, longer tapered snout |
  | **cat** | **3 images** | rounded lump with a face pasted on the end | **recognisable cat** — white chest bib, white paws, correct head and ear placement, feline posture |

  **P3.4's criterion was "does the gap between a barely-covered and a well-covered species
  narrow?"** It does not merely narrow — the barely-covered species is now *good*. And the anteater,
  which has **no corpus analogue at all**, gained facial detail that can only have come from the
  base model. That is the division of labour the stream set out to test: **species identity from
  the prompt, convention from the LoRA.**

  Both keep the convention — solid masses, hard black outlines, flat fills, unbroken bottom edges,
  correct side-profile framing. Both also have **colour** (brown anteater, grey-and-white cat), so
  the white-collapse that dogged every previous run past g10 is absent at g8.
