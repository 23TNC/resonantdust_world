# Issues — human-pawns

## I2 — `bin/art manifest` doesn't index variant-less kinds (pre-existing, out of scope)

Regenerating the corpus manifests (P1 — they were STALE, last generated before the tree
moves; pawn.rd was empty) surfaced a generator gap: a kind whose maps sit at its top level
with no numeric variant folder (`biome-tile/default/blueprint/wall/albedo.l.0.png`, the
named-form linked kinds) yields no `_var_pairs` rows and is skipped ("no mastered
variations"). Nothing consumes these corpus manifests yet (they index for the FUTURE `^r2`
resolver; the SERVING index is the edge's `tex_manifest`, which handles those kinds fine),
so this stays a note, not a fix in this stream.

## I1 — item P0.3's acceptance re-scoped (2026-07-30)

The plan asked for "a worker unit test: a same-zone hop leaves the sidecar byte-identical; a
crossing hop changes only its zone key". The built design made that test target vanish: the
worker contains NO sidecar code at all — hops are payload-free **structurally** (the only
sidecar writers are the pawn module's `spawn` and its `entity_tables!` `state_hook`, both
module-side where no host unit test can construct a `ReducerContext`). Verification instead:
(a) inspection — `grep payload` across `server/worker/` is empty; (b) the P0 live check
drills the behavior end-to-end (spawn with a marker payload, walk a zone-crossing trip, read
the sidecar back byte-identical with the zone re-keyed). The item is ticked on those two.
