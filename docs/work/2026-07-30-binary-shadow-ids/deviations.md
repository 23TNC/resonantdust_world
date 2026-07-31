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

## D2 — a new phase P2b inserted for F8 (2026-07-31)

The plan runs P2 → P3 with nothing between, because it was written before [F8](forks.md#f8) existed. F8
is not a refinement of P2; it is a **prerequisite for P3 and P4**, and it is what dissolved
[B2](blockers.md). Executing P3 against the plan as written would build the early-out on the premise
B2 proved false — that an id alone can re-find its caster.

So P2b is inserted rather than folding the work into P3's items, keeping the acceptance for "the
early-out is exact" separate from the acceptance for "a caster can be positioned at all". Two things
that can fail independently should not share a checkbox.

**One item in P2b is not F8 at all**: `VARIABLES.md`'s `shadow-cold` block still describes the pre-P1
`14 × u9 coverage` packing. P1 and P2 changed the encoding twice and never amended the doc that owns it
— so the authoritative file has been stale for two phases. Fixed here because P2b rewrites that same
block anyway, and leaving a known-false layout in `VARIABLES.md` while editing three lines below it
would be indefensible.
