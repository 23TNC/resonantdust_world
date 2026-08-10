# Blockers — shared-simulation

_Things that genuinely need human input: what blocks, why it needs a human, suggested path.
Newest-first; resolved rows archive with a date._

**None open (2026-08-10).**

Both prior rows are closed:

- **B1 — the worker clock-subscription freeze** moved to its own stream,
  [2026-08-10-sim-liveness](../2026-08-10-sim-liveness/README.md) (user: *"B1 needs a new work
  folder written and doesn't block this sequence of work"*). Correct call: it never blocked this
  stream's code, only its *measurements*, and it belongs to whoever owns the uplink. The evidence
  is carried into that stream's [I1](../2026-08-10-sim-liveness/issues.md) so this one is not the
  only copy. It stays worth knowing about here for one reason: this stream's remaining soaks
  (P4's probe re-run, P6's browserless run) can be silently invalidated by it, which is why that
  stream's P3 adds the assertion those soaks need.
- **B2 — the world could not catch up** was resolved the day it was raised: the user authorised the
  reset (*"I see no reason why I must be involved"*), it was done, and the world came back in step.
  Dropped rather than archived, at the user's instruction.

**The lesson I want to keep from B2**, because I got the call wrong: I treated resetting a dev
world as needing sign-off because a parallel session was using it. It didn't. A local world whose
state resets cheaply is not shared production, and the cost of asking exceeded the cost of being
wrong. **Confirm before destroying something expensive or irreplaceable — not before every
destructive verb.**
