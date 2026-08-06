# Issues — interactions

_Seeded at planning (2026-08-06): the user asked for the issues these systems will meet. Each is
either resolved by a fork, scoped out deliberately, or carried as a live constraint into the
items. New issues found during execution append below._

## I1 — two id regimes now coexist in one corpus {#i1}

Tiles/things are registry-numbered from taxonomy; needs/conditions — and now traits,
interactions, affordances — author explicit `id = N` because their ids are stored data
([F1](forks.md#f1)). An author adding a category has to know WHICH regime applies, and guessing
wrong is expensive in exactly one direction (a wire-riding id that got registry-numbered moves
when the registry does). P0 writes the rule into the schema section itself: **ids are explicit
iff the id rides the wire or a payload word; placeable world objects never author ids.**

## I2 — no pathfinding: seeking water is straight-line and CAN fail {#i2}

`MOVE_TO` walks greedy straight lines; the nearest water may sit behind walls and the wolf will
grind against them. Pathfinding is a declared later stream (tree-occupancy builds the occupancy
query it consumes). This stream ACCEPTS straight-line failures: the brain logs an unreached
target and re-decides, it does not block. Also latent here: the Bot exposes no tile-scan surface
yet — "nearest water" needs one (P4 builds it against the snapshot the bot already holds).

## I3 — satisfy is a read-modify-write on a LAZY value {#i3}

Nothing stores current satisfaction; the worker must compute it via `needs_eval` at now-tic,
add the effect, and write `(sat′, now)`. A concurrent `SET_NEED` landing between read and write
is clobbered. Benign today — the worker relay is the single writer for a pawn's need words — but
it is a CONSTRAINT, not a property: if writers multiply, the resolve must move inside the pawn
module's transaction. Recorded so the shortcut isn't mistaken for safe-by-nature.

## I4 — the raw SET_NEED door stays open {#i4}

`EXECUTE_INTERACTION` is worker-validated, but verb 10 (`SET_NEED`) remains a direct,
unvalidated relay any uplink client could send — the interaction gate is only as strong as that
door. Closing it means ownership/authorization (who may write which pawn), a stream of its own.
This stream builds the validated path; it does not pretend the unvalidated one is gone.

## I5 — u16 tic wrap inside the effect computation {#i5}

The worker's resolve does tic arithmetic (`set_tic` deltas, quenched expiry) in the same
wrapping u16 space as `needs_eval`. A wrong wrap either revives an expired quench or computes a
negative depletion. Rule: ALL tic math goes through the eval's helpers — no ad-hoc subtraction
at the resolve site. The crossing-drill acceptance in P3 exercises a wrap window deliberately.

## I6 — "at water" is ambiguous while water is passable {#i6}

Wolves currently walk ONTO water tiles. If water later becomes impassable (speeds/occupancy),
"standing on" silently stops ever being true. The gate therefore checks the pawn's tile ∪ its
4-neighborhood from day one, so the passability decision stays free to change without touching
the interaction system.

## I7 — double-drink refreshes quenched; it does not stack {#i7}

Grants upsert by condition id, so drinking twice quickly EXTENDS quenched's expiry rather than
doubling its mood. Chosen behavior (matches the payload's upsert law), recorded so the "why
doesn't quenched stack" question has an answer written down.

## I8 — golden fixture churn must be reviewed, not waved through {#i8}

Three new categories + two def edits (wolf traits, water affordances) re-bless the golden
fixture. The gate only guards if the re-bless diff is READ: it must be added-sections-only plus
the two expected def rows — any byte moving in an existing table is a defect, not churn.

## I9 — interactions are INSTANTANEOUS this stream {#i9}

No duration, no interruption, no "drinking" state. A timed action must negotiate with MOVE_STEP
chain supersession (what happens when a move order lands mid-drink?) and deserves its own
stream. The schema RESERVES `duration` (authored 0 everywhere) so the corpus shape doesn't
churn when that stream opens — reserving the field is cheap; building the state machine is not.

## I10 — the client-corpus filter must classify the new categories DELIBERATELY {#i10}

Server-only content is filtered by what the data IS (content-packages F2 — biome rules stay
server-side). Traits/interactions/affordances SHIP to the client: a future UI (right-click
menus, tooltips) reads them, and nothing in them is worldgen-secret. That is a decision, not a
default — the filter site gets the new categories added explicitly, with the biomes precedent
cited, so a later category doesn't inherit whatever falls out.

## I11 — the real deplete rate is unobservable live {#i11}

Thirst full→empty is 21600 tics (1 h wall); no acceptance can watch that. Drills scale via the
existing `NPC_THIRST` seed (start the wolf near a band edge); authored corpus numbers stay
production values. Every live acceptance in P3/P4 states its drill seed so "observed" numbers
are reproducible.
