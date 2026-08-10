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


## F3 — the gate missed by one tic: proceed to P3, re-measure there
**2026-08-10. Resolved: PROCEED. The box stays unticked until the number is met.**

Measured p90 = **3** tics against a criterion of ≤ 2 (n = 56, p50 = 1, 98% within 5 tics; see
[completed.md](completed.md)). The plan says "if it fails, STOP and re-plan — the design rests on
this number", so this is the moment it was written for.

**What the gate was protecting.** Not the integer 2. It asks whether a server that states an
arrival tic can be believed, because a client that lerps to a stated endpoint has no way to correct
a wrong one. Three tics at the authored paces is **0.06-0.13 of a tile** of positional error at the
destination — an order of magnitude better than what it replaces. The predecessor's live
measurement, with the pace provably correct on both sides, was a median **1.25-2.13 TILES** off at
every anchor, with 12-15% of corrections past the render-chase's give-up distance. Reading those
two numbers side by side is the whole answer.

**Why the residual exists, and why P3 removes it.** The claim and the motion are computed by two
different mechanisms today — `chord_schedule` rounds once on the running total, the per-hop chain
rounds every hop — and P1 deliberately runs them side by side without changing motion. Every tic of
the gap is per-corner and final-hop `ceil`. [P3](todo.md) makes each hop's write the literal chord
endpoint and shrinks the CONTINUE pass to a watchdog; after that the arrival IS the stated tic
because there is only one computation left. A measurement of two mechanisms agreeing to 1-3 tics is
the strongest possible evidence that replacing one with the other is safe.

**Rejected: relax the threshold to ≤ 4 and tick it.** That is rewriting a criterion to match a
result, which is precisely the failure the predecessor's tick audit caught (15 of 34 ticks false,
one criterion edited after ticking). The number is 3 and the box is open.

**Rejected: stop and re-design.** The gate's purpose is to abandon the design cheaply if the server
cannot predict itself. It predicts itself to an eighth of a tile. Abandoning here would be
discarding a design over the rounding behaviour of the code it is replacing.

**The commitment:** re-run the gate after P3's third item lands, with the criterion UNCHANGED at
p90 ≤ 2. If it does not meet it once there is a single computation, that is a real failure with no
explanation left, and the deletions in P6 must not happen.
