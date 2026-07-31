# Deviations — strip the lighting + shadow system

_Log any departure from [`todo.md`](todo.md) AT THE MOMENT of deviating, with the reason._

## D1 — P0's four measurement items struck, on the user's instruction (2026-07-31)

**What the plan said.** P0 recorded the per-pass ms at N=1 and N=16, resident RT bytes, a capability
inventory and the structural findings — all before deleting anything, on the principle that a
measurement not taken before the code goes is gone for good.

**What happened.** The user, after watching the timing harness fight the browser: _"Alright lets skip
P0."_ The four items are **struck from the plan**, not parked.

**Why this is the right call and not a shortcut.** The principle was sound; the *instance* was
over-invested. Reading it back honestly:

- **The images were the irrecoverable part**, and they are captured and committed. Everything else was
  documentation of a system that git still holds in full.
- **P1 produces the number that matters anyway.** "The frame with the lighting passes dark" is one
  frame-level measurement and it prices the whole system — which is what the replacement's budget
  needs. The per-pass breakdown was finer detail than the decision requires.
- **The harness cost was real and rising.** Five page reloads chasing WebGL timer-query retirement in
  a backgrounded tab (see [I7](issues.md#i7)); the last attempt lost 740 of 810 queries. That is a
  measurement problem, not a lighting problem, and it was consuming the stream.

**What was salvaged rather than lost** — the one clean measurement, recorded in `completed.md`: at
N=1 reach 16, gather **0.603 ms**, lighting 0.119, blit 0.378, total **1.111 ms**. It cross-checks
against the prior stream's independently-measured 0.610 ms gather, so the rig was correct.
