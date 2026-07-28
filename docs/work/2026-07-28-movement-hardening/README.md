# movement-hardening — one chain per pawn, a 6 Hz clock, no phantom despawns

_Work stream, opened 2026-07-28, immediately after [pawn-movement](../2026-07-28-pawn-movement/README.md)
delivered. Components: `shared/codec` (new worker verb + data-bit layout), `server/worker`
(chain supersession), `server/edge` (client-verb allowlist; zone-migration StateGone
suppression), `server/master` (tic pacing), `client/core`+`shared/wasm`+`client/webgl` (rate
seed), `server/gateway` (uplink migration), `client/npc` (safe re-issue). User's brief:
address the open issues and make general improvements based on this session's findings._

## The issue inventory this stream closes

- **[pawn-movement I7](../2026-07-28-pawn-movement/issues.md)** (OPEN) — `MOVE_TO` chains
  have no identity: a chain dies only by reaching ITS dest, so the npc's deadline re-issue
  while a chain is still alive spawns a SECOND chain and the two fight over the pawn.
  Observed live during the I6 deploy transition: landing errors up to 10 tiles, the trailing
  chain's `d` hovering at 1 so its "final" hops promoted EVERY slot (per-hop fan-out — the
  exact thing the cadence exists to avoid), five deadline re-issues compounding it. It
  self-drained, but the hazard re-fires whenever a deadline underestimates a slow server.
- **[pawn-movement I5](../2026-07-28-pawn-movement/issues.md)** (OPEN) — the durable tic
  advances at **5.41 Hz against the authored 6** (measured: 541 tics / 100.04 s). The
  world runs ~10 % slower than authored; every wall-time expectation (2 s/tile) is off; the
  client's learned-rate estimator absorbs it, but the server should keep its promise.
- **[first-pawns I4](../2026-07-28-first-pawns/issues.md)** (OPEN) — a cross-zone hop fires
  a phantom `StateGone` as the row migrates between zone subscriptions: the browser drops +
  re-adds the warm prim (flicker), and clients must carry "ignore removals" workarounds
  (the wolves brain keeps its adoption by policy — a REAL despawn is currently
  indistinguishable).

## Findings promoted to improvements

- **The tic estimator's ~60 s cold-page warmup**: the learned rate starts at the authored
  `TIC_HZ` and converges over ~2 re-anchor windows; until then first-trip landings correct by
  ~1 tile. A persisted rate hint kills the warmup.
- **`server/gateway/src/directory.rs`** is the last SDK-client surface NOT on
  `resonantdust-uplink` (4 `expect`/`unwrap` sites; the crate was LIFTED from this file and
  the original never migrated — recorded at sim-self-heal P6).
- **The edge accepts any parseable program from clients.** Adding a worker-only verb (below)
  makes an explicit client-verb allowlist necessary — the first, deliberately tiny,
  authorization seam.
- Docs hygiene: `docs/components/server/spacetime/modules/index/current/` has warned stale
  (stamped 07-15, code changed 07-17) in every `docs-check` this session — re-verify it.

## Design sketches (ratified as forks)

**Chain supersession ([F1](forks.md#f1), [F2](forks.md#f2)).** The seed stamps a 6-bit
trip-serial into the pawn's `data` low bits (facing owns bits 6–7; the client's
`facing(data) = data >> 6` never reads them — verified). Continuations become a WORKER-ONLY
verb `MOVE_STEP obj dest serial` (action value 8 — 1–7 are taken); a hop whose serial no
longer matches the pawn's dies silently: no step, no re-queue, one debug log. A new intent's
seed overwrites the serial, so **a new move cancels the old chain by construction** — at most
one live chain per pawn, and the npc's deadline re-issue becomes SAFE instead of hazardous.
The edge rejects `MOVE_STEP` (and other server-only verbs) in client-submitted programs.

**Phantom StateGone ([F3](forks.md#f3)).** Suppressed at the EDGE, which uniquely sees all of
a client's zone subs on one connection: track each relayed entity's current zone; a delete
for an entity now live in a DIFFERENT subscribed zone is the migration artifact — swallow it.
For delete-before-insert ordering, hold the `StateGone` relay one beat and cancel it if the
insert lands; a real removal still relays after the hold. Clients keep their defenses.

**Master pacing ([F4](forks.md#f4)).** Diagnose before fixing: instrument the loop (iteration
overhead vs `bump_tic` call latency vs the skip-when-not-landed dedup) and fix the measured
stage. Constraint: **no burst catch-up** — a tic is a wall-time promise to speculating
clients; accuracy comes from the period, never from bump bursts.

**Rate seed ([F5](forks.md#f5)).** The webgl host persists the last learned `tics_per_sec`
(localStorage, keyed by gateway) and seeds the engine's estimator at boot — a HINT, clamped
to the estimator's band; the stream remains authoritative.

## Non-goals (held intent — do not build here)

The tween-blend from speculated to authoritative position and the re-anchor-every-N knob stay
HELD (`ACTIONS.md` §Movement) — the user deferred both until the logged error data says
they're needed. Pathfinding stays behind its seam. No ownership model.

## Verification surface

`bin/sim run npc` soak + browser `:5174/?user=Claude&focus=100,50&zoom=1&cb=area1` with the
tab VISIBLE (a hidden tab freezes rAF and fakes landing errors — pawn-movement's recorded
observation); worker logs for supersession lines; `spacetime sql` for the durable tic rate
and pawn rows; forced-deadline and cross-zone drills for I7/I4.
