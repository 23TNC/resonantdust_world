# Deviations — the east pipeline

_Where execution departed from the plan, logged AT THE MOMENT of deviating. "Less churn" is never
a reason._

## D1 — the P4 run overwrote P1's measured results, because output was keyed on method alone
_2026-08-08 · caught immediately · **recovered in full**_

`east_eval` wrote to `.staging/east/<method>/results.json`. Both eval sets declare a method named
`S2a` — the trained set uses it for family-silhouette-on-known-species, the untrained set for the
only method that can serve an unknown one — so the first P4 run **silently replaced P1's trained
`S2a` measurements** with four untrained rows.

**Recovered completely** because the sprites are keyed by subject id and the two sets share none:
`--measure-only` rebuilt every trained method from the images still on disk, and the numbers came
back identical to what `completed.md` already records (S0 0.731, S1 0.852, S2a 0.829, S2b 0.725,
S4 0.850). Nothing had to be regenerated.

**Fixed** by namespacing `OUT_ROOT` on the *set*, not just the method: `.staging/east/trained/` and
`.staging/east/untrained/`.

**Why it is worth a deviation entry rather than a silent fix.** The failure was silent — the run
printed a normal-looking table of untrained rows and exited 0, and the only reason it was noticed is
that the table was full of `-` where `iou_ref` should be. Had the untrained set happened to reuse a
subject id, the sprite tree would have been clobbered too and recovery would have meant regenerating
18 cells. The general shape is the same one this project keeps meeting: **a shared namespace with no
discriminator, where the collision is invisible at the moment it happens.**
