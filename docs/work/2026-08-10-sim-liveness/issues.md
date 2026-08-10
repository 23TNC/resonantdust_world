# Issues — sim-liveness

_Problems hit, candidate solutions, which we chose and why. Chronological append._

## I1 — the evidence, carried over from shared-simulation
**2026-08-10. Open — this is the stream's subject, recorded here so the original is not the only
copy.**

Observed twice, hours apart, on the dev world. Full write-ups:
[shared-simulation I6](../2026-08-09-shared-simulation/issues.md#i6) and its
[B2](../2026-08-09-shared-simulation/blockers.md).

- Worker's `master=` reading frozen at 33461 for 12+ minutes while `rd-master` reported
  `achieved_hz` 5.9–6.2 and `bump_confirmed=360, bump_failed=0` continuously.
- Worker at ~115% CPU throughout, composing components at tics behind ones it had already passed
  (33098 at 01:34 → 32036 at 01:52).
- Onset a cliff: lag 0 ±2 for hours → 29 → 2333 on the next sample.
- Restart did not help (the backlog is in the event shard); a full state reset did.
- Recurred ~10 h after that reset, ~4,000 tics behind with the same signature.
- **Nothing alarmed, either time.** Both discoveries were accidental — an unrelated probe returning
  zero rows.

**Not yet known:** whether the subscription dies, the SDK's re-subscribe has a hole, or the
worker's loop stops draining it. P1 is exactly that question.
