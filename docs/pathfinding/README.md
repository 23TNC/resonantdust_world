# Pathfinding & movement — deterministic paths, intent rows, silent sync

How a game object gets from tile A to tile B without streaming a position update
every tic, and without any client ever seeing a jump. The scheme rests on one
idea: a **deterministic** path function in `shared` means the *intent* to move is
enough information for the worker **and** every client to reconstruct the whole
trajectory independently. We store the intent; we derive the position.

> **Status: DESIGN.** Written against the 0.2.3 tick pipeline
> ([`pipeline-generalization.md`](../components/server/spacetime/pipeline/design/pipeline-generalization.md)), the action
> model in [`shared/tick/src/domain.rs`](../../shared/tick/src/domain.rs), and the
> client sync model ([`sync.md`](../intent/sync.md)). Terminology follows
> [`object-model.md`](../components/shared/codec/design/object-model.md). Nothing here is built yet.

---

## 1. The two verbs

Today `ACTION_MOVE` (`0x100`) sets an absolute position in one tic. We split the
concept:

- **place** — *absolute*. Sets the object's position on a given tic, full stop.
  This is exactly today's `ACTION_MOVE` behaviour, renamed. It is the terminal /
  checkpoint write; it carries no notion of travel.
- **move** — *pathfind*. "Get to tile `(x, y)`." Resolving it does **not** write a
  destination position. It computes a path and writes a **move-intent** that
  describes travel over time, plus a scheduled `place` at the far end.

So the rename is: current `ACTION_MOVE` → **`ACTION_PLACE`**, and a new
**`ACTION_MOVE`** carries a destination intent. Both stay in the `Data` phase band
(`0x100..=0x2FF`); `place` keeps `0x100`, `move` takes a new id.

## 2. The core insight — the intent row *is* the position

If `path(source, dest, …)` is deterministic, then given four facts —

1. `source` tile,
2. `dest` tile,
3. `start_tic`,
4. `duration_tics` (how long the traversal takes),

— **anyone** can compute which tile the object occupies at any tic:

```
tile_at(tic) = path(source, dest)[ index_for(tic - start_tic, duration_tics) ]
```

This is the same function, byte-for-byte, on the worker (native) and on every
client (wasm). Two consequences fall out of it:

- **"What tile is the object on, with no per-tile row?"** — you never store the
  intermediate tiles. You store the intent and evaluate `tile_at(tic)`. This is
  the occupancy query, used both for game logic on the worker and for rendering on
  the client.
- **`place` is not new information.** A client that holds the intent already knows
  where the object will arrive and when. The terminal `place` therefore has two
  real jobs, neither of which is "tell the client the position":
  1. **checkpoint** — collapse the derivation into a concrete position row so
     future reads don't replay the pathfinder, and history before it can be
     truncated;
  2. **resync anchor** — a definite position for a client that joined late, or
     whose derivation drifted (it must not, if determinism holds — see §6).

## 3. The move-intent row

Resolving an `ACTION_MOVE` writes a state row describing travel, not a point. The
exact bit-layout is TBD against the object-model reference scheme, but the fields
are:

| field           | meaning                                                    |
| --------------- | ---------------------------------------------------------- |
| `object`        | the reference of the object in motion                      |
| `intent`        | `move` (distinguishes an in-flight row from a settled one) |
| `source`        | tile the traversal starts from                             |
| `dest`          | tile the traversal ends at (this segment's end — see §4)   |
| `start_tic`     | tic the traversal begins                                   |
| `duration_tics` | tics from `start_tic` to arrival                           |

`duration_tics` comes from a **deterministic speed model**: tiles-per-tic, terrain
movement cost, and diagonal cost, all integer, all in `shared` (§6). An object
with an active move-intent row and no newer `place` is "in motion"; readers
project its position with `tile_at`.

## 4. Segment, don't one-shot

Rather than compute one path across the whole map and schedule a single far-away
`place`, resolve a **segment**: a bounded run toward the destination, ending in a
`place`, followed by a fresh `move` for the next segment. A destination-only path
is just the single-segment case.

Why segmented is the default, not an optimisation:

- **Pathfinding is expensive**; bounding the horizon bounds the per-resolve cost.
- **Obstacles move** (§5). A short horizon means a stale-obstacle mistake is small
  and self-corrects at the next segment, instead of committing to a long path
  through space that won't be clear when the object gets there.
- It gives natural **re-plan points** for free.

The tradeoff is more `place` events (more checkpoints). That's cheap and, per §2,
invisible to players.

## 5. Interruption & moving obstacles — the actual hard part

The happy path (place-after-N-tics) is the easy 20%. The design work is here.

**A scheduled `place` is speculative and must be cancellable.** A new `move`
arrives mid-flight, or the object is blocked. The pending far-end `place` must not
be allowed to fire as written. On interruption:

1. evaluate `tile_at(interrupt_tic)` — where the object *actually* is;
2. write a `place` there (checkpoint reality);
3. issue the new `move` from that tile.

Concretely, the scheduled `place` is a **future-`valid_at` event that a later
write supersedes**. The resolution model must define how supersession reads so a
stale future `place` never materialises a position the object was diverted from.
This is the one piece to nail down against the bitemporal log semantics before
building.

**Moving obstacles make the pathfinder stateful.** Once two objects can path near
each other, computing object B's path needs object A's *projected* position at each
future tic — derivable (evaluate A's intent with `tile_at`), but it means the
pathfinder's input is "the static tile grid **plus every active move-intent
projected forward**," not just the grid. This is the expensive, subtle part, and
the strongest argument for short segments (§4): shorter horizons make stale
projections cheap to correct.

## 6. Determinism is the whole ballgame

If the worker and a client ever disagree on a path, the client animates one
trajectory while the server checkpoints another — a visible desync. Rules:

- **No floating point in the shared path or speed code.** The client is
  Rust→wasm; the worker is native. FP results can differ across those targets.
  A\* costs, movement/terrain/diagonal costs, tiles-per-tic, and `duration_tics`
  rounding are **integer / fixed-point end to end**.
- **One implementation, in `shared`.** Both sides link the same code (native rlib
  via the bind-mount; wasm via the `resonantdust-shared` bundle), exactly as the
  codec is shared today — so the two sides can't drift.
- **A cross-target determinism test.** Run the same `(source, dest, obstacles)`
  query natively and in wasm and diff the path. This test is the guardrail that
  keeps the sync silent.

## 7. Scheduling without polling every object

"Wait `duration_tics`, then `place`" must **wake at a future tic**, not re-check
every object every tic. The per-object tic-frontier + dirty-gating in the pipeline
is the substrate: a `move` sets the object's next wake to `start_tic +
duration_tics` (or the next segment boundary). Confirm the resolution model
supports a sparse "idle until tic T, then fire" so idle in-flight objects aren't
walked each tic.

## 8. Client rendering — why the sync is invisible

This slots straight into the existing model ([`sync.md`](../intent/sync.md)): synced
clock + shared render delay **D** + interpolate-by-`valid_at`.

- On receiving a move-intent, the client renders the object **in motion
  immediately**, driving it with the same `tile_at(tic)` the server uses. It does
  not wait for the `place`.
- Clients are slightly out of sync with each other; that's fine. The terminal
  `place` carries a `valid_at`, and pixijs **tweens** the object to the confirmed
  tile. Because the client's own derivation already had it there (or within a
  tile), the tween is sub-tile and reads as smooth motion, not a correction.
- A late-joining client needs only the current move-intent (or the last `place`) —
  not the per-tic history — to place the object correctly.

## 9. Open decisions

1. **Move-intent bit-layout** — exact packing of the row against the
   object-model reference scheme.
2. **Segment length policy** — fixed tile budget, fixed tic budget, or
   distance-to-first-obstacle.
3. **Supersession semantics** for a cancelled future `place` (§5) — how the
   bitemporal log represents "this scheduled event was revoked."
4. **Obstacle scope for v1** — static-grid-only first (ship §§1–4, 6–8), with
   moving-obstacle awareness (§5) as a second pass? Recommended, to de-risk the
   determinism work before adding the stateful pathfinder.

---

## Build order (proposed)

1. Rename `ACTION_MOVE` → `ACTION_PLACE` (pure rename, absolute set).
2. Deterministic integer pathfinder + speed model in `shared`, with the
   cross-target determinism test (§6).
3. New `ACTION_MOVE`: resolve → move-intent row + scheduled `place` (§§3–4, 7),
   static grid only.
4. Client: `tile_at` rendering from the intent + `place` tween (§8).
5. Interruption / re-path (§5).
6. Moving-obstacle awareness in the pathfinder (§5).
