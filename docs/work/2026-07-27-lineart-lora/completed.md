# Completed — verification log

## 2026-07-27 — P0 complete (decoloured corpus)

`bin/lib/decolour_corpus.py` reconstructs every sprite through the renderer's own formula with all
tints set to one flat grey, then re-applies the source alpha so the downstream prep can still crop to
the subject bbox.

**Tint chosen by measurement ([F2](forks.md#f2))**, not preference: 200 grey keeps subject luminance
in 0–200 against a 255 plate. Verified on the smoke set — beaver and anaconda both retained outline,
ear/muzzle detail and every dark pattern patch, with hue gone.

**700 of 701 sprites built.** The one failure is named and understood
([I1](issues.md#i1)) — `Elk/ElkFemale_south.png` crashes `split_layers`.

**A serious tooling defect surfaced on the way ([I1](issues.md#i1)):** `split_layers` processes all
supplied paths in one process, so a single crashing sprite aborts the run and every sprite after it
silently produces nothing. First attempt lost **417 of 701**; chunking cut that to 36; per-sprite
invocation to 1. I compounded it initially by calling it with `check=False` and suppressed stderr, so
a 60% data loss printed as a successful build. Now one invocation per sprite, with any non-zero exit
reported.
