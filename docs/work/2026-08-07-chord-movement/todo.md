# Plan — chord movement

_Items never move; `[x]` IS the move. Context + the feasibility verdict in
[`README.md`](README.md), decisions in [`forks.md`](forks.md) (F#), the
anticipated-issue inventory in [`issues.md`](issues.md) (I#)._

## P0 — the paper

- [x] ACTIONS.md §Movement: the chord law — per-chord hops (F2), mid-chord
      resolve-on-touch (F3), no start fan + the every-N re-anchor ON (F4), tiles/tic
      derived at eval (F6). Acceptance: docs-check green. → the chord law block +
      the promote cadence rewritten (seed fans nothing; REANCHOR_TICS=32 ON); green.
- [x] VARIABLES.md: pawn positions' low byte = `sx:4|sy:4` SUBTILE nibbles (F1; cold
      rows keep layer semantics); the half-open edge-ownership rule (I2).
      Acceptance: docs-check green. → the layout block shows both readings of the
      low byte + the per-shard law + half-open flooring; green.

## P1 — the codec

- [x] codec: `pack_pawn_subtile`/`position_subtile` + fractional `position_to_point`
      (tile + sixteenths); `position_to_tile` unchanged = the floor. Acceptance:
      round-trip tests; an old-style position reads subtile (0, 0). →
      `pawn_position`/`position_subtile`/`position_to_point`/`point_to_position`
      (rounding carries the sixteenth); 71/71 codec tests green.

## P2 — the route

- [x] `path_eval`: octile A* (integer costs) + string-pull on DOUBLED corners →
      chord polyline + f64 length; supercover LOS, footprint-radius arg (F5/F7).
      Acceptance: tests — minimal chords, corner rule, byte-identical determinism. →
      `find_path_octile`/`line_of_sight` (Dedu supercover, exact-corner = both
      flanks)/`find_chords` (fat-probe inflation)/`chord_len`; 13/13 green.

## P3 — the worker

- [ ] Worker: chord-chain hops — write the chord END (subtile) + queue the next at
      `+ceil(len × ground_speed)` (F2/F6); chords split at ~8 tiles (I7).
      Acceptance: a diagonal trip lands in ceil(√2·…) tics, not 2×.
- [ ] Worker: mid-chord RESOLVE-ON-TOUCH (F3) — superseding seeds, interaction
      effects, and validation floors (I2) resolve the interpolated position first.
      Acceptance: an interrupt mid-chord logs a resolved subtile start (I5).
- [ ] Worker: the seed fans NO position; bare re-anchor `PROMOTE` every
      REANCHOR_TICS=32 (F4). Acceptance: event logs show intent + re-anchors +
      landing only; no seed-position frame.

## P4 — the client

- [ ] Client: spec = the shared polyline + elapsed × tiles/tic, armed from the
      CURRENT belief (no anchor snap); authX/authY go fractional (I6). Acceptance:
      chord glide traces the polyline; landing error logged small.
- [ ] Drill (I5): interrupt a trip mid-chord with a new order — the pawn continues
      from its mid-point, server (resolved subtile) AND client (no snap-back).
      Acceptance: captures + worker resolve log + no-backtrack render trail.

## P5 — the observers

- [ ] npc + intent strip: deadlines and the walk leg's fire tics from CHORD length
      (I4); wolves soak at the lake. Acceptance: rings finish at arrival; trips
      arrive without deadline churn.

## P6 — the verdict

- [ ] Docs + memory truth pass + a stack bounce with arcs green; **the user's eyes
      close the stream**. Acceptance: docs-check green; captures + logs in
      completed.md.
