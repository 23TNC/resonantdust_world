# Client sync — authoritative state, fanned intent, tic speculation

_Intent (client/core). BUILT (first-pawns, 2026-07-28) — this doc describes the live model.
The server side is [`intent/spacetime-again`](../../../../intent/spacetime-again/README.md);
the movement contract is [`ACTIONS.md` §Movement](../../../../ACTIONS.md). Companion to
[`client.md`](client.md)._

## The philosophy (the user's, verbatim in spirit)

We do NOT sync clients with servers — lockstep was tried and could never be held. The server
fans out its **authoritative state**; clients issue commands based on whatever they think state
is; the **edge** validates + queues and the **workers** validate + execute against authoritative
state. The spacetime shards are the root of truth because they carry guarantees the rest of the
system cannot.

## Why not the alternatives (both were tried)

- **FPS netcode** (server-in-the-future, jitter buffers, prediction/rollback) exists to hide
  sub-100 ms twitch input for a handful of entities. A colony sim is the opposite shape.
- **Streaming positions** (a scheduled reducer writing a position every ~150 ms) measured 99%
  metronome-regular — with ~4.3 s scheduler stalls every ~18 s, independent of table size. No
  interpolation hides a 4 s data gap. Positions-as-samples is the wrong grain; behaviors are
  the right one.

## The live model

- **Ordering is the tic** — a wrapping `u16`, serial arithmetic only (`codec::tic`). `valid_at`
  is retired (deleted 2026-07-15). `TIC_HZ` (6) is a codec constant — the ONE authority the
  metronome, the sim loops, and the clients all read.
- **The wall↔tic estimate** (`client/core/src/ticclock.rs`): every `state`/`event` arrival
  anchors "wire tic `V` arrived at wall `W`". Arrival delay only LAGS an anchor, never leads
  it, so the estimator keeps whichever anchor implies the FURTHEST current tic. Re-anchors are
  sparse and surface as `Event::TicAnchor`, which is **DIAGNOSTIC**: hosts ask
  `Client::now_tic()` rather than re-deriving the estimate, because an instruction to rebuild a
  shared answer is how two hosts come to disagree about what time it is (shared-simulation P2e —
  npc kept its own anchor, webgl hand-rolled the conversion eight times). The extrapolation has
  one spelling, `ticclock::extrapolate`. Measured: a 90-second-old anchor predicted a fresh row's
  tic within jitter.
- **Movement is intent + speculation** (`ACTIONS.md` §Movement): a move fans ONE
  `Event::MoveIntent` (entity, dest, first-hop tic); bare continuation hops fan NOTHING; state
  promotes at the seed and final tile only. The renderer walks the pawn fractionally along the
  server's own stepping rule (`walkGreedy` mirrors the worker exactly, same
  `codec::speed::tics_per_tile`) driven by the tic estimate — smooth motion, zero per-hop
  bandwidth. Measured: a 5-hop trip = 1 intent + 2 state frames.
- **Corrections snap** (first-pawns F8): an authoritative `state` row overrides the
  speculation — the final tile clears it, an interim resolve (another event touched the pawn)
  reseeds it — and each correction logs its error (the data the future re-anchor-every-N knob
  is tuned on; measured landing error ≈ 0.01 tiles when the intent arrived).
- **Guards** (first-pawns I2/I3): the `event` table has no retention yet and re-subscribes
  replay history, so clients ignore intents with no clock, intents whose move must already be
  over, and intents serially older than the last armed one (`lastIntentTic` dedup). When an
  intent is missed entirely, the pawn falls back to seed→final snapping — correctness always
  rides state.

Best-effort by construction: the server dictates truth, the client makes it smooth.
