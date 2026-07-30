# Issues — human-pawns

## I1 — item P0.3's acceptance re-scoped (2026-07-30)

The plan asked for "a worker unit test: a same-zone hop leaves the sidecar byte-identical; a
crossing hop changes only its zone key". The built design made that test target vanish: the
worker contains NO sidecar code at all — hops are payload-free **structurally** (the only
sidecar writers are the pawn module's `spawn` and its `entity_tables!` `state_hook`, both
module-side where no host unit test can construct a `ReducerContext`). Verification instead:
(a) inspection — `grep payload` across `server/worker/` is empty; (b) the P0 live check
drills the behavior end-to-end (spawn with a marker payload, walk a zone-crossing trip, read
the sidecar back byte-identical with the zone re-keyed). The item is ticked on those two.
