# Deviations — survival

## D1 — the crossing re-stamp is a NEW VERB, not a queued SET_NEED

F4 promised "one mechanism, no new verbs" — a queued SET_NEED re-stamp.
Implementing it exposed the flaw: a queued SET_NEED carries a LITERAL packed
value computed at queue time; at fire it would write the PREDICTED zero
blindly, killing a pawn that ate meanwhile. Re-validation must happen AT
PROCESSING — so the worker gains `RESTAMP_NEED` (worker-only, never in
CLIENT_VERBS): `[RESTAMP_NEED, pawn, need_reference]`, whose arm evaluates
the need's CURRENT lazy value fresh, writes it, and feeds the same
death-sweep lane a SET_NEED write feeds. Everything else in F4 stands: one
pending slot per (pawn, need), earliest-wins scheduling, the removal hook,
the horizon clamp, and the existing sweep does the killing. The full
new-verb ritual (spawn-authority I1) applies.
