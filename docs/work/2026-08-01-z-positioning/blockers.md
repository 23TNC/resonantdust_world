# Blockers — z positioning

_No open blockers._

## 2026-08-01 · P2's `unit.z` ambiguity — considered, NOT filed

Worth recording that this nearly became a blocker. [F9](forks.md#f9) found `unit.z` holding a world
height for lights and a drawn shift for the head — a real problem, and one whose fix changes shipped
lighting by ~10%.

**It is still not a blocker.** The fix is one line in `RecordSync`, trivially reversible, and the
user's own stated rule (`screen.y = unit.y − unit.z`) picks the option. "This changes shipped
behaviour" is a reason to say so loudly in the fork and the commit — not a reason to hand back.
