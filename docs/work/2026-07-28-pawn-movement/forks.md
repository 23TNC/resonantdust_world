# Forks — pawn-movement

_Decisions resolved (or leaned) at plan time; each names the rejected options and why._

## F1 · Speed unit — RESOLVED BY THE USER (2026-07-28): tics per tile, authored in the DSL

**Supersedes first-pawns F7's wall-time authoring.** F7 authored speed in tiles/second and
converted through `TIC_HZ` so a tic-rate change never rescaled the world. The user's ruling:
"We will define the speed in tics per tile. We won't measure in seconds, as our time is
measured in tics." So the corpus authors the tic count directly (wolf = 12 → 2 s/tile at 6 Hz)
and a `TIC_HZ` change DOES change wall-clock movement speed — accepted, because tics are the
game's time unit and content should read in it. `codec::speed` keeps only the DEFAULT for
unauthored kinds and the resolution rule; the per-kind value is content.

*Authoring idiom (leaned, settle at build):* whatever the loader already does for per-def
scalars — a `:data` hook returning the value or a `&thing.speed set`, mirroring how `thing.span`
is authored (`2 &thing.span set`). Do NOT invent a new parser form for one number.

## F2 · Command shape — destination-only, ALREADY BUILT; keep fanning the seed

The user suspected asking `MOVE_TO` to accept source+destination was a mistake and
destination-only (source = current position) was right. Investigation: the built verb is
already destination-only — `MOVE_TO obj dest`, source READ from authoritative state at each
hop (`ACTIONS.md`: "a mid-move `PLACE` isn't overrun — `src` is not an operand, it's read").
**No verb change.** On the user's open question — fan the starting position or not: it is
already fanned, and stays, because it rides the NORMAL state channel (the seed hop is
`PROMOTE MOVE_TO`, promoting the position like any other state write), not a special operand
or message — so it costs the verb's generality nothing and gives every client the speculation
anchor. Rejected: dropping the seed fan (clients late to the zone would speculate from a
stale position for the whole trip).

## F3 · Where the worker reads content speed — disk corpus, not an edge fetch

The worker loads `content/*.rd` from disk at startup (`CONTENT_DIR`, default
`/workspace/content` — the sim containers already mount the repo), mirroring the edge's
`read_content_dir` file order so def-ids agree. Rejected for now: fetching `/content` from the
edge like the npc does (the worker has no login flow to learn the edge URL, and it adds a
liveness dependency the sim tier doesn't need); hot-reload/R2 for deployed workers is a
recorded follow-on — dev speed changes arrive with a redeploy anyway. The def-id agreement
risk (worker's disk corpus vs edge's served corpus drifting mid-session) is accepted in dev
and noted for the deployed story.

## F4 · Snap now, tween later — RESOLVED BY THE USER (2026-07-28)

Authoritative `State` overrides speculation by SNAPPING to the server's tile (the existing
first-pawns F8 path: landing clears the spec, interim resolves reseed, every correction logs
its error). The speculation↔authoritative tween-blend is FUTURE INTENT — recorded in
`ACTIONS.md` §Movement as a held knob beside re-anchor-every-N, tuned later on the logged
error data. Not built this stream.

## F5 · One speed authority, keyed by `object_id`

All three consumers key speed by the content `object_id` (= the pawn's `definition_reference`
for CREATE-minted pawns): worker + npc resolve through the corpus, webgl through the bundle
table. `codec::speed` holds only `DEFAULT_TICS_PER_TILE` + `resolve(Option<u16>)`. The old
stub signature `tics_per_tile(def)` is DELETED (delete, don't deprecate) so no caller can
silently keep the constant path — the compiler finds every consumer. Rejected: keeping codec
as a lookup that takes the def id (codec must not depend on the corpus; the corpus is content,
codec is wire/math).
