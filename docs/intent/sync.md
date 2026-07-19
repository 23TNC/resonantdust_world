# Client/server sync — authoritative bitemporal event log + deterministic projection

> **0.2 note (2026-07-09):** the **server** model below is superseded by the tick
> pipeline (`server_master`/`server_edge`/`server_simulation`, event/state split,
> per-object tic frontier). The **client** model here — synced clock, render delay
> `D`, interpolate-by-`valid_at`, gray-out — is **retained**.

**Status:** Design locked (2026-07-08); substrate partly built. Live today: clock
sync, client interpolation, and a **stopgap** debug mover that streams a position
every 150 ms. Not yet built: **Slice 0**, which converts motion from
position-streaming to future-stamped per-tile `free_things` rows and makes the
client projector speed-aware. This doc is the plan for how every client stays in
sync with the server for a RimWorld-like MMO simulation, and the incremental path
to get there. Companion to [`client.md`](../components/client/core/intent/client.md).

## The problem

The old game ran "server several seconds in the future, clients various seconds in
the past" — the Source/Overwatch pattern: interpolate a few continuously-moving
entities behind a jitter buffer, predict the local avatar, reconcile. That exists
to hide sub-100 ms input lag on *twitch* input for a *small* number of entities. A
colony sim is the opposite shape — server-authoritative, clients are thin viewers
of a *subset* of zones, no twitch input — so almost all of that machinery is wrong
for us.

The first attempt streamed positions: a scheduled reducer wrote a loose thing's
position every 150 ms; the client buffered the rows by `valid_at` and interpolated.
It measured **99% metronome-regular** (median 152 ms) but with **stalls up to ~4.3 s,
roughly one every 18 s** — and the stall rate was **independent of table size**
(pruning history to ~20 rows didn't change it). Conclusion: the SpacetimeDB
scheduled-reducer mechanism stalls periodically on its own, and **streaming
positions through it is fundamentally unviable** — no client interpolation hides a
4-second data gap; you get freeze-then-jump.

That failure is what forces the real model.

## Why this model, and not another

For an MMO-scale simulation the candidates are:

- **Deterministic lockstep** (StarCraft; RimWorld's own MP mod). Every client runs
  the identical full sim, exchanging only inputs. Minimal bandwidth, perfect
  consistency — but every client must simulate the *entire world*, it advances only
  as fast as the slowest player, and join/leave is painful. That's exactly why
  RimWorld MP is co-op-for-a-few, **not** an MMO. Partial views kill it.
- **Server-authoritative state replication** (WoW, Source). Server ticks the world,
  streams snapshots/deltas, clients interpolate/predict. Scales to partial views —
  but bandwidth scales with *entities × update rate* (the wall we hit). To make it
  work at sim-entity counts you bolt on interest management + delta compression +
  tiered tick rates… which is slowly reinventing an event log by hand.
- **CRDTs / eventual consistency.** Give you *convergence*, not *authority* or
  causal simulation order. A sim needs a canonical outcome and anti-cheat. Wrong
  tool.
- **Authoritative event log + deterministic projection** (this doc). Bandwidth
  scales with *decisions*, not entity-count × rate — and a colony sim is mostly
  decisions (pawns follow jobs and paths; walls don't move). Supports partial views
  and join/leave. It's the only model that fits all three.

It is **not** a silver bullet, and the honest caveats shape later slices:

1. **It's a hybrid.** Chaotic subsystems — fire spread, gas/temperature diffusion,
   fluids — don't compress into sparse intents; they generate dense events and
   degrade the log back toward streaming. The escape is to simulate those
   **deterministically on each client from a seed + periodic authoritative
   checkpoints**, not as logged events.
2. **Determinism is a real tax**, and it scales with how much you project. Linear
   motion is trivially reproducible; client-side fire simulation across
   native/wasm/different CPUs is where float drift and unlogged RNG bite. Keep the
   projected math integer/fixed-point and the determinism surface small.

## The time model

Two lines on one synced game-time axis:

- **`A` — the authoritative edge.** The server's simulation frontier. It has
  validated and committed everything with `valid_at ≤ A`. **Nothing exists past A.**
  This is "the horizon."
- **`R` — the client render line** = `syncedNow − D`, where `D` is the shared render
  delay. Where a client displays.

**The invariant is `R ≤ A`, always.** Clients *always* trail the authority; nothing
a client renders is ahead of the authoritative edge. (This corrects an earlier
sloppy framing of "scheduling facts past the horizon" — a fact's `valid_at` can be
in the client's *future* (`R < valid_at ≤ A`), ahead of where the **client**
renders but still behind the **authoritative edge**. The server committed it; the
client buffers it and reaches it as `R` advances.)

The gap `A − R` is the **safety window** (target ~10 s), sized to *max client lag +
validation time*. Player-action effects are committed into `(R, A]` — never into
already-rendered history (`< R`), which is **frozen and immutable**. That
immutability is what makes deterministic projection hold and bounds validation to
the window.

`D` today is **300 ms** (`RENDER_DELAY_MS` in `pixijs/src/client/WasmClient.ts`),
one value for every client — a shared `D` off a shared clock is what makes two
clients render the *same instant*. It is also the command-latency budget.

## Facts, not positions — the grain shift

This is the whole change, and it's subtle because the tables *already look like* an
event log: bitemporal `valid_at` PK, history per key, reap-to-latest GC. The
machinery doesn't change. **What a row means** does.

- A `free_things` row today is a **state sample**: "object X *is at* L *at* time T."
  A point on the timeline. To know where it is at time T you need a row near T →
  continuous motion needs continuous rows.
- An event-log row is a **behavior**: "object X *is heading to* L, arriving at time
  T." A time-parametric segment. To know where it is at any time you **evaluate**
  it → one row covers a whole tile traversal.

Same `valid_at`; the row goes from a *sampled point* to a *described segment*. One
segment replaces the thousands of samples continuous motion would otherwise write.
Two consequences: **write on change, not on tick** (~100× fewer writes — the fix for
the scheduler stalls), and the **client projects** (`state(T) = f(log, T)`) instead
of reading-latest.

## Movement, concretely

Movement lives entirely in the **active-things** layer, which already exists:

- `cold_zones` — settled, tile-snapped, baked per region. **Static.**
- `free_things` — region-free, carry their own position, bitemporal. **The moving
  layer.** A moving thing is an active thing whose `location` changes over time.

**No new table.** A "plan" is just several `free_things` rows written ahead: the
server pathfinds, then commits one row per upcoming tile at its future `valid_at`,
keeping a couple of tiles of **look-ahead**. Because the next tiles are already
committed and buffered on the client, a 4-second scheduler stall no longer starves
it — the client keeps projecting from rows it already holds.

### Where is a thing while moving?

Two positions, answering to different masters:

- **Logical occupancy** — the one tile the *sim* says it's on, for reservation,
  pathing, "what's on this tile," anti-gridlock. This is the **current row's
  location**. Committing to a move frees the source tile immediately (so a follower
  can enter it — retain the source and conga-lines gridlock) and reserves the
  destination. This is what RimWorld does (`Position` snaps to the next cell at
  move-start; `DrawPos` interpolates).
- **Visual position** — the interpolated sub-tile point, tweening *from* the
  previous tile *toward* the current one.

So: **the to-tile owns the logic, the from-tile owns the pixels.** They're different
questions, not a compromise.

### No duration field — the projector is speed-aware

Two rows are enough to describe A→B-*wait*-B→C (one row for B, one for C). No
duration field, no marker rows. The naive failure — **"smearing"** — is what happens
if the projector lerps blindly across the *whole* gap between two rows:

> Rows `B @ 1000 ms`, `C @ 3000 ms`, one tile = 500 ms to cross. Truth: sit at B
> from 1000→2500, then cross 2500→3000. A naive lerp across `[1000, 3000]` puts the
> thing **halfway to C at 2000 ms**, when it should still be *on* B — the 1.5 s wait
> smeared into slow drift.

Two *position* timestamps alone are ambiguous ("waited then crossed" vs "crept the
whole time" produce identical rows). The tie-breaker is **travel time**, computed
from the thing's **speed + terrain cost** — which the client must reproduce anyway
for deterministic projection. So the projector reconstructs the move: crossing takes
500 ms → the B→C move occupies `[3000 − 500, 3000]`; before 2500 it **holds** at B,
then lerps. No smear, no extra data on the row. The **authoritative arrival snaps it
true** — if the client's travel-time estimate is ever slightly off, the next row
corrects it.

The invariant behind this: to separate *moving* from *resting* you need one of
(a) the movement **speed** (compute the duration, hold the remainder — chosen here),
(b) a **second timestamp per move** (depart-row + arrive-row), or (c) a **duration
field**. They're informationally equivalent; speed is the cleanest because it's a
stat the client already needs and never has to be spelled out per row.

## Garbage collection

GC stays — "event log" never meant append-forever, and unbounded append is exactly
what stalled us. It just becomes a non-event:

- **Same rule as today** — keep the latest self-contained fact per key. A new
  behavior supersedes the old, like a new position row superseded the old.
- **What changes is cadence.** "Latest" now turns over per *tile change* (seconds),
  not per 150 ms, so the table stays tiny and snapshots stay cheap.
- **Retention floor:** latest fact per key, plus the last ~2 rows per object (the
  visual tween needs the "from" tile), plus anything still valid in the window
  `(A − maxlag, A]`. Bounded.
- **Self-contained vs delta facts:** reap-to-latest only works for self-contained
  facts (a behavior or absolute state that fully replaces its predecessor). Delta
  facts ("took 5 damage") need periodic folding into a checkpoint (the
  event-sourcing "snapshot the read-model" step). **Prefer self-contained facts**;
  add checkpointing only where deltas are unavoidable.
- **Resync boundary:** a client that falls *more than the window* behind
  (backgrounded tab, sleep) has had its needed past reaped. It **resyncs** — snap `R`
  forward to `A − window`, read the latest fact per key, project forward. No deep
  history needed — which is why "latest fact per key" is the floor GC never drops.

## Determinism — the linchpin

`state(T)` must be a **pure function of the immutable, totally-ordered log and T**.
The classic killers are cross-machine float drift and unlogged RNG. Mitigations,
locked in from the start:

- Projected math in **integer / fixed-point**.
- Total order is already given by `valid_at = (time_ms << 16 | seq)` — the `seq`
  tiebreak means no two facts are ever ambiguously ordered.
- **Seed and log every RNG draw** so a projecting client reproduces it.
- Facts below `R` are immutable; corrections live only in `(R, A]`.

## What exists today

Built, deployed, building green:

- **Clock sync** — NTP-style, best-RTT-wins, in `client/src/clock.rs`; ping/pong
  frames in both `protocol.rs`; server replies in `server/src/ws.rs`; both engines
  (`engine.rs`, `web.rs`) ping every 2 s and seed from `login_ok.server_micros`;
  marshaled to the `clockSync` JS event; `WasmClient.syncedNowMs()`; the debug HUD
  sync tab is live.
- **Client interpolation** — `WorldBridge` buffers positions and tweens to
  `syncedNow − D`; `Viewport.movePrim`; delete-handling that only drops a sprite
  when *all* versions are gone (a GC/same-ms delete removes one version, not the
  thing).
- **Stopgap debug mover** — `object_shard/src/debug_mover.rs`, still streaming a
  position every 150 ms with history pruning. This is what Slice 0 replaces; it's
  the source of the periodic stutter.

## Build path

Each slice ships and is verifiable on its own.

- **Slice 0 — motion as future-stamped per-tile facts.** *(Next.)* Server emits one
  `free_things` row per tile at its future arrival `valid_at`, with a couple of
  tiles of look-ahead; the 150 ms streaming mover is **deleted**. Client projector
  goes **speed-aware** (tween over the computed travel window, hold the rest);
  logical occupancy = current row. Touch points: `object_shard` (writer + delete
  the scheduled position mover), `WorldBridge` (speed-aware projection). Kills the
  stutter and stands up the projection substrate. Verify: two browsers, glass-smooth
  *identical* motion, provably immune to the 18 s stalls (server writes per tile,
  not per frame).
- **Slice 1 — generalize the substrate.** A fact-kind → projector registry on the
  client and the canonical apply loop (apply facts with `valid_at ≤ R`, buffer the
  rest; fast-forward late/replayed facts). Lock the determinism rules while it's
  small.
- **Slice 2 — authoritative sim loop + horizon.** A server tick that advances `A`
  ahead of `R` by the safety window and *emits facts* (sparse intents) rather than
  positions — so scheduler jitter is harmless (`A` just advances in lumps; as long
  as `A ≥ R + margin`, clients never starve).
- **Slice 3 — client actions + validation.** Client sends an intent; server
  validates against projected state at effective time; commits a fact in `(R, A]` or
  rejects. The action→validate→commit→project loop, one entity.
- **Slice 4 — full sim + rolling-window validation + chaotic-subsystem hybrid.**
  Interacting entities, pathfinding/jobs/conflicts, "all states in the window
  valid," and the seeded-local-sim + checkpoint fallback for fire/gas/temperature.

## Deferred / open

- **Presence under future-stamping.** `free_things::place` reads `latest(ctx)` at
  server-now, which lags future-stamped rows; region presence counts could drift for
  a thing whose future moves are committed ahead. Fine for a single-region mover;
  needs a time-aware presence update for the general case.
- **Chaotic subsystems** (fire/gas/temperature) — seeded local sim + checkpoints,
  Slice 4.
- **`TILE_TRAVEL_MS` is duplicated** in `debug_mover.rs` and `WorldBridge.ts` (like
  the existing `SQUARE = 64`), a stopgap until move speed becomes a data-driven
  per-thing stat.

## Done

- **Old drift constants retired** (2026-07-08). The dead `effective_now_ms` +
  `BACKWARD_GRACE_MS` / `TIME_DRIFT_BUFFER_MS` / `MAX_RTT_MS` are deleted from both
  shards' `time.rs`, and the `players` login back-stamp (`CLIENT_RENDER_BUFFER_MAX_MS`,
  sized to the old `clientDelay` range) is gone — login reducers stamp at server-now.
  The render delay is now purely the single client-side `D`.
