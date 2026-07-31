# Deviations — binary occlusion + stored caster ids

_Log any departure from [`todo.md`](todo.md) AT THE MOMENT of deviating, with the reason._

## D1 — P2's items rewritten after F7 (2026-07-31)

P2 was planned as "allocate an id map at shadow resolution, two px per texel for 16 x u16" — a second
storage location beside the coverage texel. Three attempts to build that failed ([I6](issues.md),
[I6b](issues.md), [I6c](issues.md)), and [F7](forks.md#f7) replaced the premise: at 8 slots the ids fit
in the texel that already exists, and the coverage value is deleted rather than stored alongside.

The old items describe work that should not be done, so they are rewritten rather than left to mislead a
resuming session. The phase's GOAL is unchanged — record which caster occluded each slot — only its
storage changed.
