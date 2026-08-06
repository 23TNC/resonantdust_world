# Issues — input-rework (anticipated inventory)

## I1 — speculation continuity: the MoveIntent channel must keep fanning {#i1}

The client speculates ONLY on `MoveIntent`s manufactured from fanned `MOVE_TO` seeds
(`client/core/world.rs::move_intents` — MOVE_STEP hops never reach clients). With the seed now
queued by the WORKER at `master+4` instead of sent by the client, the intent's `event_tic`
shifts and the seed rides a server-queued event row. Verify LIVE: a menu-driven move produces
exactly one intent per trip and the glide runs — before trusting anything downstream.

## I2 — chain supersession under the new seed's event_reference {#i2}

`apply`'s MOVE_TO arm stamps the trip serial from `event_reference & 0x3F` — now the
worker-queued effect event's reference, not the client intent's. Supersession (re-issue kills
the old chain) must be re-drilled: two rapid menu moves → ONE surviving chain.

## I3 — the migration ORDER inside the stream: callers first, the door last {#i3}

npc and client swap to the interaction door BEFORE `MOVE_TO` leaves `CLIENT_VERBS` — the
reverse order strands them mid-stream. The edge allowlist change is its own item, deliberately
after both callers' items, and the rejection drill (a raw MOVE_TO queued and refused) runs
only then.

## I4 — the menu needs the ACTING pawn's rows client-side {#i4}

Availability = predicates over the active pawn's trait/condition/needs rows — all already
fanned to `MoverLayer` (payload + needs). A pawn selected before its payload frame lands has
no rows yet → predicates over an empty set → `can_*` fails → no menu. Acceptable (the window
is one fan), but it must degrade to NOTHING, never to a throw or a wrongly-offered option.

## I5 — DOM menu vs canvas input: execution wins over dismissal {#i5}

"ANY input action dismisses" must not eat the menu's own click: a pointerdown on a rect
executes (then hides); a pointerdown anywhere else hides. The menu is DOM above the canvas —
its clicks must not fall through to `selectAt`/pan underneath, and the canvas handler must not
dismiss-then-reopen on the same left click that spawned it. Wheel/keys/middle/right all
dismiss.

## I6 — the destination input is a `position_reference` {#i6}

`destination` rides the event as the packed u32 `tile_to_position(x, y)` — the same word the
old MOVE_TO carried, so zone-crossing trips inherit the chain's existing per-hop routing.
The worker validates the DESTINATION tile's carrier binding by the same per-biome merged
baseline ⊕ overlay read the drink arm uses (one row per biome — merge, never first-found).

## I7 — the speed-deletion sweep has many consumers {#i7}

`speed = 12` reaches: the worker (`load_corpus` speeds + `tics_for`), the npc
(`resolve_thing_in`, trip deadlines), wasm (`thingSpeed`), MoverLayer (`speedFor`), the golden
dump, the loader (`thing_speed`/`thing_speeds`, sim fingerprint), and content. Delete-greps
are part of the acceptance; the fingerprint change bumps thing sim versions (expected — dev
re-mints). `DEFAULT_TICS_PER_TILE` survives ONLY as the chain-spacing fallback for a
degenerate derived 0 (which the gate should make unreachable).

## I8 — golden churn again {#i8}

menu_text, the move effect, location `"target"`, carrier lists on five tiles, drink's new
signature, and the speeds section leaving. Same discipline: extend, re-bless once, account for
every line.

## I9 — menu targets are tiles and things this stream; pawns are later {#i9}

Left-clicking a PAWN as the target (social interactions) has no corpus content and no
hit-test-to-carrier path yet — the menu offers nothing for pawn targets this stream. Things
use `thing_interactions` (empty today — a waterskin later). Recorded, not built.

## I10 — the wolves brain must not fight the player's menu orders {#i10}

A menu-driven move and the npc's wander both issue trips for the same wolf (no ownership —
interactions I4 still open). Chain supersession makes the LAST order win mechanically, but the
npc re-issues on its deadline — a player-ordered wolf will wander off afterward. Accepted this
stream (ownership is the recorded successor); the drill just needs to expect it.

## I11 — FOUND LIVE: universal carriers broke the npc's drink filter {#i11}

With `move_to` on every ground tile, the wolves brain's "first usable interaction on this
tile" matched GRASS: standing anywhere, it fired `move_to` through its drink-shaped
3-input composer (`input count does not match the corpus signature` at the worker) and
latched `drink_issued` — the wolf wandered Thirsty forever. Two fixes: the drink pass
filters to interactions that SATISFY (has a `satisfy` effect), and `fire_interaction` binds
by the F5 reserved vocabulary (`pawn`/`amount`) instead of the hardcoded three-input drink
shape — the same generalization the pie menu composer uses, which the drill then re-proved.
