# Mover perf — N dark movers, the worker's ceiling characterized — 2026-08-08

**What** (user, 2026-08-08): another performance test. A `debug_mover` pawn —
just a pawn that MOVES. Convert the npc light-mover module into a DEBUG
module; it moves 8, 16, and 24 debug movers around, measured at 1× zoom. We
will likely catch the workers falling behind — they have a lot of work
updating all 24 movers — at which point we have a **stable platform to debug
and improve off of**.

**Why**: torch-perf's row 3 caught the worker 24 tics behind at 25 movers —
from ONE sample, which cannot distinguish a fixed pipeline delay from an
unbounded deficit. And those movers carried lights, so the row conflates two
loads. This stream isolates pure movement (no light), and its deliverable is
the LAG CHARACTERIZATION over time — the reproducible baseline any future
worker improvement is measured against.

**Design stance**:
- `debug_mover` = `debug_torch` minus the light ([F1](forks.md#f1)): walks 2,
  no needs, a flat tint part. The torch-perf rows become the LIT twin — same
  counts, same pen, same pace — so lighting's server-side cost (expected:
  none) falls out by comparison ([F4](forks.md#f4)).
- The `torches` brain CONVERTS to a generic `debug` brain ([F2](forks.md#f2)):
  `NPC_KIND` names the pawn kind to procession (debug_mover, debug_torch, …),
  `NPC_COUNT` the population; the `torches` name DIES (delete, don't
  deprecate — one debug harness, parameterized).
- The PRIMARY metric is the master−worker tic lag OVER TIME
  ([F3](forks.md#f3)) — the worker's own compose lines carry the pair
  (`composed component tic=X master=Y`), so the sampler is a log read, no new
  tooling. The 24-row soaks ≥10 minutes; the verdict is STABLE vs GROWING
  ([I2](issues.md#i2)). Client `__framecost` at zoom 1 rides along as the
  secondary.

**Exit**: the three-row table with lag-over-time series and the
stable-vs-growing verdict, the lit-vs-dark comparison read, the debug harness
in place as the repeatable platform, world restored, docs+memory truth pass;
the user's eyes close the stream. Improving the worker is explicitly NOT this
stream ([I7](issues.md#i7)) — this buys the ruler.
