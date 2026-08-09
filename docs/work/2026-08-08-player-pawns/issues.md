# Issues — player-pawns (anticipated inventory)

## I1 — the entity_reference namespace + event routing must be decided FIRST

Events target entities by packed reference; the orchestrator routes claims by type to shards.
Player-pawn entities need a reference lane the whole event system (edge allowlist →
orchestrator claim → worker write) routes to the NEW shard without colliding with pawn refs.
The P0 paper pins the packing (own type id vs a subtype discriminator) BEFORE any code — a
wrong namespace here is a migration, not a fix.

## I2 — the full module ritual applies, plus one

A new spacetime MODULE is heavier than a new verb: publish + orchestrator routing + worker
uplinks + edge subscription plumbing + `rd`/`bin` deploy wiring, and the sim self-heal
(rebuild-on-next-use) must cover the new DB. Budget it; the stale-module symptom (a redeploy
that silently serves the old wasm) is a known foot-gun — verify with a live probe after every
publish.

## I3 — every eval consumer needs the player-pawn's rows delivered

The ONE eval runs in worker, wasm/webgl, and npc. Worker reads the shard directly; wasm/webgl
and npc need the OWNER fan (F6) wired through the edge + client/core event surface before any
brain can evaluate its own wolf_count. The consumer sweep (rebuild + restart everything on
the new corpus AND the new event surface) is a standing cost — plan it, don't discover it.

## I4 — "one active" needs one enforcement point

Even pinned to one-per-player, the ACTIVE bit should be enforced in exactly one place (the
login/selection path in the auth DB), so the future multi-character selection changes one
funnel. Anything reading "the player's player-pawn" goes through the linkage — never "the
first row that matches".

## I5 — npc-host is downstream; its folder must say so

npc-host's I10(a) (player need storage) is ANSWERED here; its F7 tag classification is
SUPERSEDED by F4 subtypes; its F8 wolf_pawn need becomes `wolf_count` on the player-pawn.
Update its folder when this stream lands (and at plan time, mark the dependency) so a session
executing npc-host doesn't re-decide or contradict this stream.

## I6 — login gains a write; keep it idempotent and non-blocking

Minting at login puts a spawn on the hot login path. The spawn-log dedup makes replays safe,
but login must not FAIL when the playerpawn shard is momentarily down (self-heal window):
degrade to "no player-pawn yet, retry next login/use" rather than blocking the session.

## I7 — visibility for the drills

Nothing renders a player-pawn (F3), so the drills need eyes: the SQL probe (HTTP SQL on the
playerpawn DB) is the baseline; a debug panel read (the browser session showing its own
player-pawn's needs/conditions) is the stretch. State which one each drill uses up front.

## I8 — the needs machinery's pawn-isms will surface

The need-write sweep, crossing scheduler, and death lane were built for pawns — death/removal
semantics make no sense for a player-pawn (what dies?). The copy inherits the lanes; the P0
paper states which are ACTIVE for player-pawns (bands/conditions/emotions yes; death sweep
NO — a zeroed wolf_count must never remove the player-pawn).
