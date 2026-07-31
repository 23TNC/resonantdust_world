# Deviations — lighting + shader rework

_Log any departure from [`todo.md`](todo.md) AT THE MOMENT of deviating, with the reason._

_None yet._

## D1 — P0's harness acceptance cited a stale baseline (2026-07-31)

**Plan:** "reproduces the stripped baseline of **0.045 ms/frame** within its own spread."

**Reality:** 0.045 was the strip's **P1** figure. Its P2 and P4 took the floor to **0.028** by deleting
`moverDirty`'s per-frame record rewrites and the cursor light — both found after this plan was written.

**Resolution:** the harness was checked against the *current* floor (0.035 / 0.028 across two runs,
each inside the other's spread), which is what the acceptance was actually for. Logged rather than
quietly re-baselined, because a plan number silently edited to match a measurement is how a harness
gets trusted for the wrong reason.
