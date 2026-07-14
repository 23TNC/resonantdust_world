# Intent — pathfinding (cross-cutting, staged)

_Staged feature intent — components not yet fully ascribed. Last updated: 2026-07-14._

## What we want (the intent)

A client asks to go to a destination tile; the **server pathfinds** and **commits one move per
upcoming tile ahead of time**, keeping a couple of tiles of **look-ahead**. Because the next tiles
are already committed and buffered on the client, a scheduler stall doesn't starve rendering — the
client keeps projecting from rows it already holds. (Source: `docs/sync.md` — "the server
pathfinds, then commits one row per upcoming tile at its future `valid_at`, keeping a couple of
tiles of look-ahead.")

## Current status

**Not implemented.** `moveTo` today resolves a **direct** move to the requested tile (edge appends
one `ACTION_MOVE`; the worker resolves it) — no route, no per-tile look-ahead. The intent above is
the *target*, not the present behavior.

**Caveat:** the source (`docs/sync.md`) predates the event-DSL rewrite and is written in the old
`free_things` / `valid_at`-rows vocabulary. The concept survives; the mechanism must be
**re-expressed in the current model** before it's a contract — roughly: pathfind → append **one
`MOVE` event per tile at successive future tics**, with a few tics of look-ahead, so the client's
interpolate-by-`valid_at` renderer stays fed.

## Components it touches (to distribute intent into, once scoped)

- **client/pixijs + client/core** — the `moveTo` request (destination tile) + interpolating the
  buffered per-tile moves.
- **server/edge** — receives the destination, runs the pathfind, appends the look-ahead move
  events (today it appends a single direct move).
- **shared/tick** — the `MOVE` domain effect + any route/step representation.
- **server/worker + shard** — resolves the per-tile `MOVE` events into `state` at their tics.

## Open questions (why it's still staged, not distributed)

- Where does the pathfind actually run — edge, worker, or a new piece? (`sync.md` says "the
  server"; unresolved against the current split.)
- Look-ahead depth vs the `TIC_GAP`/drop-on-miss causality — how many tiles/tics ahead is safe?
- Route representation: N separate `MOVE` events, or one multi-step program (the event-DSL
  supports multi-action vectors now)?
- Recompute/interrupt on a new `moveTo` or a blocked tile.

Resolve these (with the human) → distribute this intent into the component `intent/` folders above
and retire this staging entry to a pointer.
