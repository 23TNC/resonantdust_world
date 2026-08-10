# Blockers — shared-simulation

_Things that genuinely need human input: what blocks, why it needs a human, suggested path.
Newest-first; resolved rows archive with a date._

**None open (2026-08-09).**

Everything the stream has hit so far is a decision I could make and did — recorded in
[`forks.md`](forks.md) F1–F5 — or a problem with a chosen path, recorded in
[`issues.md`](issues.md) I1–I5. The stream is executable as written.

One thing to flag rather than block on, because it is the user's call and not mine: **P4 deletes
working code.** `MoverLayer`'s speculation is the only reason movers glide today, and it carries
guards earned live (the stale-intent and older-row rejections — the teleport verdict). The plan
ports those into `client/core` at P2 and proves the track headless at P3 before P4 removes
anything, and [F4](forks.md#f4) argues against keeping a flagged fallback. If you would rather
carry the TS path behind a flag for a release, say so and F4 gets rewritten — it is a reversible
decision and cheap to change *before* P4, expensive after.
