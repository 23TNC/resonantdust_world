# Forks — server-chords

_Decision points, options, which we took and why. Chronological append._

## F1 — inherited: the client gets no opinion
**2026-08-10.** Carried from [shared-simulation F9](../2026-08-09-shared-simulation/forks.md#f9),
which is the founding decision and should be read there. Summary: a shared rule still leaves two
extrapolators whose agreement must be re-earned on every edit; two stated endpoints and a lerp
cannot drift. And rooting motion to server-issued straight lines BOUNDS the failure — a client that
falls behind skips along a line the server also believes in.

## F2 — `CANCEL` is a new verb, not `CANCEL_INTENT`
**2026-08-10.** Tempting to widen `CANCEL_INTENT` (14) with `entry_id 0 = "all"`.

**No.** Its signature is `[Imm, Imm]` — the pawn is not a write operand, so the verb neither joins
the pawn's write group nor serialises against its in-flight hops, and it writes nothing. F9's
cancel must PLACE the pawn at its resolved position. A signature cannot be conditional on an
operand's value, so widening it is not expressible. New id, `&[ReadWrite]`.

## F3 — prove the arrival tic before deleting anything
**2026-08-10.** The design's load-bearing claim is that a stated destination tic is TRUE. Nothing
in the codebase demonstrates the server can predict its own arrival: today it recomputes each hop
and discovers the arrival when it happens.

So [P1](todo.md) fans chords **alongside** the existing chain and changes no motion, gated on
`|stated − observed| p90 ≤ 2 tics` over 50 trips. Deletions are last. This ordering costs one
throwaway integration and buys the ability to abandon the design for the price of a `git revert`
rather than a rebuild of both clients.

The predecessor is the argument for this: it deleted the TypeScript walk on a premise
([I1](../2026-08-09-shared-simulation/issues.md#i1)) that later turned out to be a measurement
artifact of my own making. Measure first this time.
