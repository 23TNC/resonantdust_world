# deviations — teardown-housekeeping

Where the code departs from the plan (`docs/components/<c>/{design,intent}`). Log a row **at the
moment you deviate**, not when someone catches it.

The plan encodes decisions we spent real thought on, so a departure is how bugs get in — it must
carry a **strong** reason. *"Less churn", "the existing code already did X", and "it's only
cosmetic" are not reasons* — they are the absence of one. If the plan looks wrong, change **the
plan** (with input); don't quietly diverge in code.

Rows: date · what the plan says · what the code does · why · fix/status.

_None yet._

**Watch for one in particular:** the P0-gates-P2/P3 ordering is the stream's single load-bearing
constraint, and it is the kind that gets skipped under momentum ("the prune is obviously safe"). If
a phase runs out of order, that is a deviation and it gets a row here — not a shrug.
