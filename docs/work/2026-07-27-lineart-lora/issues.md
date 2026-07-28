# Issues

## I1 — `split_layers` aborts the whole invocation on one bad sprite (silent mass data loss)

`Elk/ElkFemale_south.png` raises `IndexError: index 0 is out of bounds for axis 1 with size 0`
inside `split_albedo`. Because `split_layers.py` processes every supplied path **in one process**,
that exception kills the run and **every sprite after it silently produces no output**.

Measured cost while building the decoloured corpus:

| invocation shape | sprites processed | lost |
|---|---|---|
| all 701 in one call | 284 | **417** |
| chunks of 40 | 665 | 36 |
| **one call per sprite** | **700** | **1** (the genuine crasher) |

Isolated by running the 36 survivors individually: **35 were fine** — they were collateral damage
from sharing a batch with the crasher.

**Why this was dangerous:** the failure is *silent*. The tool exits non-zero but prints per-sprite
successes for everything before the crash, so a caller that doesn't check the return code (mine
didn't at first — `check=False` plus suppressed stderr) sees a plausible-looking partial corpus and
no error. A 60% data loss looked like a successful build.

**Handled** in `decolour_corpus.py` by invoking split_layers once per sprite and reporting any
non-zero exit. Slower, but a crash can only lose its own image.

**Still open:** the underlying crash. Some property of `ElkFemale_south` produces an empty axis
during clustering — likely a sprite whose regions all fall below `--min-region`, leaving zero
material candidates. Worth a guard in `split_albedo` so it degrades to "no layers" rather than
raising.
