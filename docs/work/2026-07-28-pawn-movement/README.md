# pawn-movement — content-authored speed + speculation that actually glides

_Work stream, opened 2026-07-28. Components: `shared/dsl` (speed authoring), `shared/codec`
(speed seam), `content/` (wolf def), `server/worker` (continuation spacing), `server/spacetime`
(`event_shard` retention), `client/webgl` (MoverLayer speculation), `client/npc` (trip
deadlines), `docs/ACTIONS.md`. User's brief: ensure pawns MOVE CORRECTLY. Speed becomes a
DSL-authored property in **tics per tile** (wolf = 12 → 2 s/tile at 6 Hz); the client speculates
from the fanned intent (start tic + start location + destination + tics-per-tile) and stays in
sync WITHOUT per-tic position updates; the server fans the real position at the destination and
authoritative state overrides speculation — first pass **snaps** to the server's tile, a
speculation↔authoritative tween is a recorded LATER. And the observed misbehaviour — pawns
snapping somewhere, tweening, snapping somewhere else — gets root-caused and fixed: if movement
is queued correctly, pawns should be tweening along speculated paths, full stop._

## User decisions this stream encodes

- **Speed is measured in tics per tile, authored in the DSL.** "We won't measure in seconds, as
  our time is measured in tics." Wolf = **12** tics/tile (2 s/tile at 6 Hz). This SUPERSEDES
  first-pawns F7's wall-time authoring ([F1](forks.md#f1)).
- **Snap to the server's tile for now** — authoritative state overrides speculation; the
  tween-from-speculation-to-authoritative blend is future intent, recorded not built
  ([F4](forks.md#f4)).
- **Command shape is mine to choose.** The user suspected asking for source+destination was a
  mistake — investigation shows the built verb is ALREADY destination-only ([F2](forks.md#f2)):
  no change needed, and the seed fan-out stays.

## What exists (verified in-code 2026-07-28 — build on it)

- **The verb is already the shape the user wants.** `MOVE_TO obj dest` takes NO source —
  `ACTIONS.md`: "reads `obj`'s current position (so a mid-move `PLACE` isn't overrun — `src` is
  not an operand, it's read)". The initial program is `PROMOTE_EVENT PROMOTE MOVE_TO obj dest`
  (`client/core/src/world.rs::move_to_program`); the worker self-queues bare continuations at
  `tic + tics_per_tile` and the final hop is `PROMOTE MOVE_TO`. So the client already receives:
  the **intent** once (dest + event tic), the **seed** position as an authoritative `State` row
  (start anchor), and the **landing** as another. The start position IS fanned — via the normal
  state channel, not a verb operand, so the verb stays general ([F2](forks.md#f2)).
- **The speculation layer exists and mirrors the server's stepping exactly.**
  `MoverLayer.walkGreedy` = the worker's greedy line; progress = `ticDelta(eventTic) /
  ticsPerTile`; authoritative rows snap/reseed and log the error (first-pawns F8). When a trip's
  intent arrives and arms, the glide is proven (landings at e=0.01).
- **The speed seam is ONE function but a STUB**: `codec::speed::tics_per_tile(def)` ignores its
  argument — every kind walks at `WALK_TILES_PER_SEC = 2.0` → 3 tics/tile. Its three consumers
  are the worker (continuation spacing), MoverLayer (speculation rate), and the wolves brain
  (trip deadline). All three must read the SAME per-def value or speculation drifts by design.
- **DSL defs already carry per-kind authored data** (`:data`/`:visual` facets, hooks run by
  `shared/dsl/loader`; the corpus is the def-id authority). The wolf def is
  `content/data/things.rd :: wolf` — data-side empty today. The client gets per-kind tables from
  the content bundle (`thing_layout()`, `thing_light()` …) — speed joins as a sibling table.
  The edge serves `/content` from disk (dev) or R2 (deployed) with a hot-poll; the npc already
  fetches and loads the corpus; the worker mounts the repo (`/workspace`) in its container.

## What's WRONG (the snap-tween-snap, diagnosed)

The server side is clean — first-pawns measured single chains at exact `tics_per_tile` spacing,
exactly one `event` row per trip. The commands are NOT random; the visible teleporting is the
CLIENT failing to arm speculation, so a trip degrades to its two authoritative snaps
(seed → landing) with no glide between. Three concrete drop paths in `MoverLayer.onMoveIntent`
plus two inherited server gaps:

1. **Intent for an unseen mover is dropped** (`if (!m) return`) — the comment claims the seed
   `State` "reseeds everything", but a dropped intent is GONE: the seed State only places the
   pawn; no spec is ever armed. Any trip whose intent outraces the pawn's first State row (page
   load ordering, spawn-then-move) renders as two snaps.
2. **Intent before the tic clock anchors is dropped** (`d === null ⇒ return`) — same outcome on
   fresh loads.
3. **Stale-replay guards can eat live intents' arming** only via the paths above; the guards
   themselves (finished-long-ago, serial dedup) are correct but are DEFENSE for…
4. **first-pawns I2** — the `event` table has NO retention: every (re-)subscribe replays all
   history. Re-subscribes are routine (anchor hysteresis), so replayed stale intents keep
   arriving forever; guarded client-side today, unfixed server-side.
5. **first-pawns I3** — intent delivery at the edge was measured FLAKY (absent/doubled per
   trip). An `on_applied` replay on the edge's event sub landed during sim-self-heal as a
   mitigation; never re-measured under soak.

A missed intent + a later stale replay arming a WRONG spec (old dest, old tic — passes the
guards if the clock estimate is loose) is exactly "snap somewhere, tween, snap somewhere else."

## The plan's shape

P1 authors speed in content (docs first: ACTIONS/codec doc rules change — tics, not wall-time).
P2 plumbs the ONE value to all three consumers (worker via disk corpus [F3](forks.md#f3), webgl
via bundle table, npc via its fetched corpus) keyed by `object_id` ([F5](forks.md#f5)).
P3 kills the drop paths (pending-intent buffer) and the replay source (event retention sweep),
then re-measures I3 under soak. P4 verifies the whole story in the browser at 12 tics/tile and
wraps. Fix bugs in webgl only; pixijs is gone.

Verification surface: `bin/sim run npc` (wolves soak) + browser
`:5174/?user=Claude&focus=100,50&zoom=1&cb=area1` — `[mover]` console lines carry armed intents,
landings, and reseed errors; worker logs carry hop spacing.
