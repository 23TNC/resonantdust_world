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
