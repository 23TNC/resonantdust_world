# Plan — shared-simulation

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions in
[`forks.md`](forks.md) (F#), the issue inventory in [`issues.md`](issues.md) (I#)._

**How acceptance is measured.** Rust: `bin/sim check <crate>` / `bin/sim test <crate>` and
`bin/rd build core`. Browser: `npm run typecheck` + `npm run build` in `client/webgl`, plus the
live fluffle at `:5174/?user=Claude&focus=124,75&zoom=1` read through the console probe P0 builds.
Server behaviour: `bin/sim logs worker` and `spacetime sql resonantdust-dev-pawn-0`.

**Phase order is load-bearing.** P4 deletes the client's only way to move a pawn between anchors,
so P2 must have landed. Do not reorder.

## P0 — the measurement, so the fix is provable

- [ ] Extend `__teleportProbe` with a divergence tally: per-kind reseed error and tiles-per-anchor,
      kept as sorted samples. Acceptance: one console read prints p50/p90 for both, per kind.
- [ ] Make the probe survive a page reload by parking samples in `sessionStorage`. Acceptance: a
      reload mid-soak keeps the running counts.
- [ ] Record a 10-minute baseline off the live fluffle into [`completed.md`](completed.md).
      Acceptance: a dated row with reseed p50/p90, anchor stride p50, RENDER-teleports per minute.

## P1 — the shared module; the worker becomes a caller

- [ ] Create `shared/content/src/move_eval.rs`, exported from `lib.rs`, holding `REANCHOR_TICS`
      and `CHORD_CAP_TILES`. Acceptance: `bin/sim check worker` green.
- [ ] Move `hop_stride_tiles` into `move_eval` verbatim. Acceptance: unit test covers the clamp at
      both ends — pace 1 → 8 tiles, pace 240 → 1 tile.
- [ ] Add `move_eval::next_hop(from, dest, pace, pathable) -> (landing, tics)` carrying the chord
      step, the `clear_point_fraction` clamp and the recenter. Acceptance: unit test reproduces 20
      landings recorded from the live worker.
- [ ] Add `move_eval::position_at(from, dest, base_tic, now, pace, pathable) -> point` — where a
      walking pawn IS at any tic. Acceptance: at the hop's own tic it equals `next_hop`'s landing
      for the same 20 cases.
- [ ] Rewrite worker `MOVE_STEP` and `resolve_walk_position_for` as calls into `move_eval`, and
      delete the private copies. Acceptance: `grep -c 'REANCHOR_TICS\|CHORD_CAP_TILES' server/`
      is 0.
- [ ] Re-run the P0 probe against the rebuilt worker. Acceptance: anchor stride p50 within 0.05
      tiles of the P0 baseline — a pure move changed nothing.

## P2 — `client/core` holds positions

- [ ] Add a `movers` track to `client/core`: entity → last authoritative point + tic, active
      intent, pace. Fed from `StateObject` and `MoveIntent`. Acceptance: `bin/rd build core` green.
- [ ] Derive each mover's pace in core through the shared `stat_eval`, never a constant.
      Acceptance: core reports 24 tics/tile for a bunny and 12 for a wolf.
- [ ] Expose `pawn_point(entity, now_tic)` over the track via `move_eval::position_at`.
      Acceptance: a headless run logs a moving pawn's point changing between two anchors.
- [ ] Port the replay guards the TS earned — the stale-intent and older-row rejections
      (`MoverLayer.ts:754-776`, `:839`). Acceptance: unit test — a minutes-old intent is rejected.

## P3 — the headless clients see motion again

- [ ] Replace npc's tile-only `pawns` map ([`lib.rs:266`](../../../client/npc/src/lib.rs)) with
      reads of core's track. Acceptance: no `HashMap<u32, (i32, i32)>` pawn store remains.
- [ ] Re-check the wolf chase against live subtile positions. Acceptance: a wolf closes on a
      moving bunny without overshoot across a 10-minute soak.
- [ ] Re-check bunny forage adjacency, which asks cheb ≤ 1 of a moving pawn. Acceptance: forage
      completions per minute do not drop against the P0 baseline.

## P4 — webgl becomes a display

- [ ] Expose core's track to the browser through `shared/wasm` as `pawnPoint(entity, nowTic)`.
      Acceptance: at an anchor tic it equals the point that anchor's row carries.
- [ ] Point `MoverLayer`'s render-chase at `pawnPoint` instead of its own `Spec`. Acceptance:
      typecheck + build green and movers still glide between anchors.
- [ ] Delete `Spec`, the walk, `speedFor`, `computePath` and `SPEC_APPLY_EPS` from `MoverLayer`.
      Acceptance: zero references remain; the file drops under 800 lines.
- [ ] Re-run the P0 probe for 10 minutes. Acceptance: reseed p50 under 0.5 tiles and zero RENDER
      teleport events — [I1](issues.md#i1) closed by construction, or reopened loudly.

## P5 — the guard, so it cannot come back

- [ ] Add a `rd docs-check` rule failing on a simulation constant or stepping rule defined outside
      `shared/`. Acceptance: it fires on a deliberately reintroduced copy, green otherwise.
- [ ] Write the simulation-vs-presentation line into `docs/components/client/webgl/intent/`:
      shared answers move, webgl only smooths them. Acceptance: docs-check link integrity green.

## P6 — the exit

- [ ] Confirm [I2](issues.md#i2)'s pre-fan fallback is gone — core knows the pace before it moves
      anything. Acceptance: no mover ever reports `DEFAULT_TICS_PER_TILE` during a cold load.
- [ ] User watches the fluffle at `:5174/?user=Claude&focus=124,75&zoom=1`. Acceptance: no bunny
      visibly jumps.
- [ ] Write the exit record and the memory line. Acceptance: docs-check green.
