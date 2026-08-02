# Deviations — beat e07

_Where the code departs from the plan (`components/<c>/{design,intent}`). Log a deviation AT THE
MOMENT of deviating, with the reason. "Less churn" is never a reason._

None yet.

## D1 — P0.5 was a plan error: it ran the OLD model to "establish a bar"
_2026-08-02 · caught by the user mid-execution_

The item read *"Re-run the frozen A/B on `e07` to confirm the bar still reproduces."* I wrote it
into the same folder as [F2](forks.md#f2), which states that attribution is explicitly not a goal
and that the stream exists to build the best model, not to compare against the old one. The two
contradict each other and I executed the wrong one.

**The user's correction:** *"We do not need to run anything using the old models … we are focusing
on generating a new lora with the new hardware the best we can with the lessons learned. I have no
clue why we need to be dicking around with the old models we are going to be replacing."*

**Cost:** ~36 generations plus the reconstruction of the seed sweep — roughly 20 minutes of GPU
and a chunk of the session, for a number nothing downstream needs.

**Kept, because it was already paid for and costs nothing further:** `e07` on the reconstructed
ruler scores **gate 35/36 · `iou_ref` 0.740 · composite 65.3** (per species: pig 0.857, bear 0.839,
tiger 0.709, cat 0.702, wolf 0.671, fox 0.662). Recorded as a footnote, not a gate. Note it does
NOT reproduce the historical "26/36 · 0.753" — expected, since [I6](issues.md#i6) established that
driver no longer exists and this is a reconstruction.

**Also trimmed at the same time:** [P1](todo.md)'s base probe no longer includes cyberrealisticXL.
The live decision is Illustrious vs Animagine; the incumbent is being replaced, not evaluated.

**What was NOT waste:** [P0.4](todo.md)'s pinned eval set. It is what the NEW model's per-epoch
sheets and any final comparison read, and pinning it exposed three live defects
([I6](issues.md#i6), the dead `round()` path, the 240 s cold-load timeout).
