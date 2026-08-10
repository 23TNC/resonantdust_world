# Plan — server-chords

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions in
[`forks.md`](forks.md) (F#), problems in [`issues.md`](issues.md) (I#)._

**How acceptance is measured.** Rust: `bin/sim check <crate>`, `bin/rd build core`, and
`cargo test` in the sim builder image. Browser: `npm run typecheck` + `npm run build`, plus the
live world at `:5174/?user=Claude&focus=124,75&zoom=1&ambient=0.6`. Headless: the read-surface
probe, `./client/core/target/release/headless <name> anchor <x> <y>`.

**File order IS execution order.** The one hard rule: **P1 and P2 come before any deletion.** The
design rests on a stated arrival tic being true and on chord counts fitting the queue; both are
measurements, neither has been taken, and the deletions are irreversible.

**A ticked box means its criterion was RUN.** The predecessor's tick audit found 15 of 34 ticks
false, including one whose criterion had been rewritten after ticking. Do not repeat that.

## P0 — the wire

- [x] Add `MOVE_CHORDS = 20` and `CANCEL = 21` to `shared/codec::action` and to the palette in
      `docs/ACTIONS.md`. Acceptance: `docs-check` green; ids follow the append-only law.
- [x] Give `CANCEL` the signature `&[ReadWrite]` (obj) so the pawn is a write target and the verb
      groups and serialises against its own in-flight hops. Acceptance: a unit test asserts the
      arity and that `collect_operands` yields the pawn as a write.
- [x] Add `MOVE_CHORDS` to the variable-arity framing list and the `collect_operands` skip list.
      Acceptance: a program with a verb AFTER a `MOVE_CHORDS` still frames; omitting the skip
      panics at the `signature().expect()`, so a test covers it.
- [x] Add `MOVE_CHORDS` to `target_routes`' write-less arm and give the worker the same
      pawn→macro_position fan lookup `QUEUE_STATE` has. Acceptance: the fan reaches a subscriber
      rather than completing zoneless.
- [x] Admit `CANCEL` to the edge's `CLIENT_VERBS`; keep `MOVE_CHORDS` out. Acceptance: a browser
      `CANCEL` is accepted and a browser `MOVE_CHORDS` is refused as malformed.
- [x] Rebuild the codec bind-mount and REDEPLOY the event-shard module. Acceptance: a hand-queued
      `CANCEL` survives the shard's re-validation (the intent-queue-ui lesson).

## P1 — can the server predict its own arrival? (no motion changes, no deletions)

- [x] Lift the hop tic schedule (`main.rs` CONTINUE pass, `k = ceil(dist × pace)`) into
      `move_eval::chord_schedule(from, chords, pace, start_tic) -> Vec<(position, tic)>`,
      accumulating on the RUNNING total so per-chord `ceil` cannot drift the arrival.
      Acceptance: a unit test — 10 chords' last tic equals `start + ceil(total_len × pace)` ±1.
- [x] Prepend the pawn's resolved SUBTILE point as the schedule's source; `find_chords` returns
      tile centres and a mid-walk pawn is not on one. Acceptance: a test with an off-lattice start
      shows chord 0's source equal to that point, not its tile centre.
- [x] Clamp any first chord whose tic falls inside the event shard's `TIC_GAP` barrier to the
      floor rather than dropping it. Acceptance: a short first chord still fans.
- [x] Fan `MOVE_CHORDS` at the walk-composing site, ALONGSIDE the existing per-hop chain, changing
      no motion. Acceptance: headless decodes a chord list for a walking pawn.
- [ ] **The gate:** measure `|stated dest tic − observed StateObject arrival tic|` over 50 trips.
      Acceptance: p90 ≤ 2 tics, recorded in `completed.md`. If it fails, STOP and re-plan — the
      design rests on this number.

## P2 — measure the three unknowns before committing

- [x] Histogram `find_chords` over 1000 random pairs × 3 distance bands on the live map.
      Acceptance: a table in `completed.md` including the **fraction needing >10 chords** — that
      fraction is the re-request stutter rate.
- [x] Measure player-order latency today: wall ms from command to first rendered displacement,
      p50/p90, n=50. Acceptance: a baseline row to compare the cancel-first path against.
- [ ] Run the mover-perf harness at 24 movers with a `BUILD_WALL` burst. Acceptance: a compose-lag
      series against the existing baseline, so P7's interrupt scan has something to regress.
- [ ] Decide N from those numbers and record it. Acceptance: a resolved fork naming the chord cap,
      whether it is `INTENT_CAP` or a separate `CHORD_CAP`, and where truncation happens.

## P3 — the server precomputes the route

- [ ] Hold the stamped chord list on `Running::Move` so cancel, resolve-from-tic and re-request
      answer from worker memory. Acceptance: no re-path on a cancel.
- [ ] Queue the whole chord list up front and shrink the CONTINUE pass to a watchdog.
      Acceptance: one route computation per order, verified by a counter over a 5-minute soak.
- [ ] Make each hop's write the LITERAL chord endpoint rather than a re-derived landing.
      Acceptance: every authoritative row equals a stated chord endpoint exactly.
- [ ] Keep stride-cadence anchor writes UNDER the chords, or prove the client's tic estimate
      survives without them. Acceptance: `TicEstimate` anchor age p90 unchanged vs the P2 baseline
      — see the README's watch list; this is the invariant most likely to bite.
- [ ] Re-point the worker's own `active_dest`/`position_at` users at the chord list. Acceptance:
      adjacency on a moving pawn (cheb ≤ 1) still passes mid-chord.
- [ ] Fire arrival on the schedule rather than polling tile-vs-dest, and make a TRUNCATED queue
      advance rather than hang. Acceptance: a parked interaction behind a truncated route still
      completes.

## P4 — CANCEL

- [ ] Implement `CANCEL`: bump the trip serial, resolve position from the tic, PLACE it under
      `PROMOTE`, drop the queue and fan the empty `QUEUE_STATE`. Acceptance: a cancel mid-chord
      leaves the pawn at its interpolated position, not its chord's end.
- [ ] Compose `CANCEL` + the new order as ONE program at the fresh-order site, so both land in one
      event and one group. Acceptance: no window in which the pawn has neither route.
- [ ] Widen the trip serial past 6 bits, or prove aliasing cannot resurrect a dead chain now that
      cancel churn is routine. Acceptance: a test that a cancelled chain stays dead across 64+
      cancels.
- [ ] Delete "queue cleared, glide finishes" — the behaviour the design forbids. Acceptance:
      `grep` finds no path where a cancelled walk keeps moving.

## P5 — the client interpolates

- [ ] Add `Event::MoveChords` and fold it in `ClientWorld::observe_event`, carrying the payload
      verbatim. Acceptance: a core test brackets `now` and lerps to a known point.
- [ ] Replace `MoverTrack`'s leg/pace/dest machinery with the stamped chord list; `point_at` picks
      the bracketing chord and lerps, holding at the source before the first tic and at the
      destination after the last. Acceptance: it NEVER returns a point outside the stated
      endpoints.
- [ ] Re-key the replay guards onto the chord frame — a replayed bundle is as poisonous as a
      replayed `MOVE_TO`. Acceptance: an older bundle does not roll the route back.
- [ ] Point the render chase's rate at the ACTIVE CHORD's own speed rather than a derived pace.
      Acceptance: the chase still smooths, and no `pawnPace` caller remains.
- [ ] Expose queue depth and the re-request threshold as a core answer. Acceptance: the client's
      only remaining decision is WHEN to ask, and both hosts read the same number.

## P6 — the deletions (irreversible; only after P1 and P2 hold)

- [ ] Delete `move_eval::position_at`, `advance_along` and `first_leg`. Acceptance: `grep -c` is 0
      outside the worker's own resolve.
- [ ] Delete `Event::MoveIntent` and every consumer across core, npc and webgl. Acceptance: the
      speculation channel is gone from the contract.
- [ ] Delete the wasm `findChords`/`findPath` exports. Acceptance: no browser-callable pathfinder
      exists to re-grow a second walk from.
- [ ] Delete `MoverLayer`'s `Walk` record, `onMoveIntent`, `pendingIntents` and `pathProbe`.
      Acceptance: `MoverLayer.ts` holds no route and no intent subscription.
- [ ] Delete the divergence tally and the AUTH teleport probe (~200 lines). Acceptance: gone —
      they measure whether two extrapolators agree, and there is one stated answer now.
- [ ] Delete `WALK_STALE_TICS` and the phase-1 "busy forever" branch. Acceptance: the last chord's
      destination tic is the end tic, so the bound has no job.

## P7 — a wall stops the pawn

- [ ] Factor `supercover(a, b, radius)` out of `path_eval::line_of_sight` so the interrupt index
      and the pathability validation cannot disagree about which tiles a chord touches.
      Acceptance: a test that both agree on a diagonal.
- [ ] Build the tile → live-path index from the chords' supercover. Acceptance: a query answers
      "who is walking through here" in one lookup, not an event-log scan.
- [ ] Hook `collect_cold_overlay` to issue `CANCEL` for paths crossing a changed, pathability-
      relevant cell. Acceptance: a wall dropped mid-route stops that pawn; a cosmetic overlay
      interrupts nobody.
- [ ] Re-run P2's `BUILD_WALL` burst. Acceptance: compose lag within the P2 baseline — a perimeter
      of SETs must not re-plan the world.

## P8 — the law, and the exit

- [ ] Rewrite `docs/ACTIONS.md` §Movement: it currently declares stateless per-hop recompute and
      "no stored route" as LAW, and this inverts it. Acceptance: docs-check green, no doc
      contradicting the code.
- [x] Add the missing palette rows for `INV_ADD` 15, `INV_REMOVE` 16 and `ACTIVATE_TRAIT` 19, found
      absent while allocating 20/21. Acceptance: the table matches the codec.
- [ ] Re-measure the predecessor's reseed table. Acceptance: reseed p50 under 0.5 tiles and zero
      RENDER teleports over a foreground soak — the number the whole line of work chased.
- [ ] Write the exit record and the memory line. Acceptance: docs-check green.
