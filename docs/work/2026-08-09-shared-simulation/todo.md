# Plan — shared-simulation

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions in
[`forks.md`](forks.md) (F#), the issue inventory in [`issues.md`](issues.md) (I#), and the
2026-08-10 tick audit in [`tick-audit.md`](tick-audit.md)._

**How acceptance is measured.** Rust: `bin/sim check <crate>` / `bin/sim test <crate>`,
`bin/rd build core`, and `cargo test` in `client/core` via the sim builder image. Browser:
`npm run typecheck` + `npm run build` in `client/webgl`, plus the live world at
`:5174/?user=Claude&focus=124,75&zoom=1&ambient=0.6`. Server: `bin/sim logs worker`.

**Phases run in the order below — file order IS execution order.** The 2026-08-10 amendment
originally appended its phases at the end, which sent a resuming session at P3 before the read
surface P3 depends on; they are merged into place here and `rd work brief` follows the same order.

**The rule the tick audit bought us:** a box goes `[x]` only when its stated criterion has been
RUN, not when the work looks done. Fifteen of thirty-four ticks failed that test on 2026-08-10 —
including one whose criterion I had rewritten after ticking it.

## P0 — the measurement (the regression check, not a gate — [F6](forks.md#f6))

- [x] Extend `__teleportProbe` with a divergence tally: per-kind reseed error and tiles-per-anchor,
      kept as sorted samples. Acceptance: one console read prints p50/p90 for both, per kind.
- [x] Make the probe survive a page reload by parking samples in `sessionStorage`. Acceptance: a
      reload mid-soak keeps the running counts.
- [x] Record a 10-minute baseline off the live fluffle into [`completed.md`](completed.md).
      Acceptance: a dated row with reseed p50/p90, anchor stride p50, RENDER-teleports per minute.

## P1 — the shared module; the worker becomes a caller

- [x] Create `shared/content/src/move_eval.rs`, exported from `lib.rs`, holding `REANCHOR_TICS`
      and `CHORD_CAP_TILES`. Acceptance: `bin/sim check worker` green.
- [x] Move `hop_stride_tiles` into `move_eval` verbatim. Acceptance: unit test covers the clamp at
      both ends — pace 1 → 8 tiles, pace 240 → 1 tile.
- [x] Add `move_eval::next_hop(from, dest, pace, pathable) -> (landing, tics)` carrying the chord
      step, the `clear_point_fraction` clamp and the recenter. Acceptance: tests cover the clamp
      and the stride cap; the RECENTER is carved out to [I10](issues.md#i10), which owns proving
      it reachable or deleting it.
- [x] Add `move_eval::position_at(from, dest, base_tic, now, pace, pathable) -> point` — where a
      walking pawn IS at any tic. Acceptance: `position_at_the_hop_tic_equals_the_hop_landing`
      passes — sampling the walk at the hop's tic lands exactly where the hop lands.
- [x] Rewrite worker `MOVE_STEP` and `resolve_walk_position_for` as calls into `move_eval`, and
      delete the private copies. Acceptance: `grep -c 'REANCHOR_TICS\|CHORD_CAP_TILES' server/`
      is 0 (run with `-r`; without it grep exits 2 on a directory and prints a misleading 0).
- [x] Re-run the P0 probe against the rebuilt worker. Acceptance: anchor stride p50 within 0.05
      tiles of the P0 baseline — a pure move changed nothing.

## P2 — `client/core` holds positions

- [x] Add a `movers` track to `client/core`: entity → last authoritative point + tic, active
      intent, pace. Fed from `StateObject` and `MoveIntent`. Acceptance: `bin/rd build core` green.
- [x] Derive each mover's pace in core through the shared `stat_eval`, never a constant.
      Acceptance: core reports 24 tics/tile for a bunny and 12 for a wolf.
- [x] Expose `pawn_point(entity, now_tic)` over the track via `move_eval::position_at`.
      Acceptance: a headless run logs a moving pawn's point changing between two anchors.
- [x] Port the replay guards the TS earned — the stale-intent and older-row rejections
      (`MoverLayer.ts:754-776`, `:839`). Acceptance: unit test — a minutes-old intent is rejected.

## P2b — the composed WORLD VIEW moves into `client/core` ([F7](forks.md#f7))

- [x] Add a `world` view to `client/core`: baseline tile kinds per zone ⊕ cold overlays ⊕ the
      composed thing view, fed from the `ColdTiles`/`ColdThings`/state events core already
      receives. Acceptance: `bin/rd build core` green.
- [x] Expose `pathable(x, y)` on it from the corpus flags, and give `MoverTrack` its probe
      internally. Acceptance: the track's `observe_*` no longer take a `pathable` argument.
- [x] Decide and pin the unstreamed-cell rule in ONE place — the client reads unknown as OPEN, the
      worker does not. Acceptance: a unit test asserts the chosen rule; the divergence is logged
      in [`issues.md`](issues.md).
- [x] Replace `client/npc`'s own `tiles`/`tile_overlays`/thing view with reads of core's.
      Acceptance: no composed-view maps remain in `client/npc/src/lib.rs`.

## P2c — the read surface — a host ASKS core

- [x] Put `ClientWorld` behind a shared cell the `Client` handle can READ, on both hosts
      (`engine.rs:63-65`, `web.rs:63-65` are `cmd_tx`-only). Acceptance: a core test drives a row
      through the engine and reads `pawn_point` off the handle.
- [x] Feed that `ClientWorld` from the engine's OWN decode — every arm that emits a state, cold or
      zone event observes into it first. Acceptance: `grep -c ClientWorld` in `engine.rs` and
      `web.rs` is nonzero for both.
- [x] Give core the corpus: fetch `/content` at login into an `Arc<Bundle>` on the engine.
      Acceptance: `derive_paces` and `pathable` no longer take a `&Bundle` argument from the host
      (`movers.rs:222`, `world_view.rs:140`).
- [x] Delete npc's hand-rolled fold — the `world` field (`lib.rs:267`) and the six `Event::` arms in
      `Bot::note` (`lib.rs:358-420`) become reads of the handle. Acceptance:
      `grep -c world_view::WorldView client/npc/src/lib.rs` is 0.

## P2d — the pawn's gameplay ROWS are core's answer ([I8](issues.md#i8))

- [x] Move `payload`/`needs` off `Mover` (`movers.rs:66-69`) into an entity-keyed store on
      `ClientWorld` that accepts rows for entities with no position row yet. Acceptance: unit test —
      `PawnParts` arriving BEFORE `StateObject` still derives pace 24.
- [x] Key need rows through `resonantdust_codec::object::row_reference` (`object.rs:164`), deleting
      the open-coded modulo at `movers.rs:207`. Acceptance: `grep -rn "0x1_0000_0000\|0x100000000"`
      over `client/` returns 0.
- [x] Give the store ONE eviction rule — drop on `StateGone`/removal and on zone close, mirroring
      `MoverLayer.ts:1371-1372,1383-1384`. Acceptance: unit test — closing a zone drops that zone's
      rows to 0.
- [x] Expose `pawn_payload(entity)` / `pawn_needs(entity)` on the handle, needs as native
      `(u64, u16)` pairs. Acceptance: a headless run prints a live bunny's thirst value nonzero.
- [x] Delete `payloads`/`need_rows` from the brains (`wolves.rs:90,93`, `bunnies.rs:53-54`,
      `debug.rs:48`) and read core's. Acceptance: `grep -c "payloads\|need_rows"` over
      `client/npc/src/brains/` is 0 — the `< 64` cap goes with them.

## P2e — one clock — core answers "what tic is it"

- [x] Add `now_tic()` to the read surface off `ticclock::delta_since` (`ticclock.rs:182`) — its
      first production caller ever. Acceptance: `grep -rn delta_since client/core/src` returns a
      non-test caller.
- [x] Demote `Event::TicAnchor` to DIAGNOSTIC and delete the extrapolation formula from
      `api.rs:236-240` — the instruction IS the leak. Acceptance: no host-facing doc tells a host to
      extrapolate; docs-check green.
- [x] Delete npc's `tic_anchor` (`lib.rs:261`, stamped at `:374`) and `Bot::now_tic`'s local
      extrapolation (`lib.rs:464-468`); its five call sites read core's. Acceptance:
      `grep -c tic_anchor client/npc/src/lib.rs` is 0.
- [x] Have npc call the `SeedTicRate` seam (`api.rs:93`) at login from its last observed rate — no
      headless host has ever called it. Acceptance: `grep -rn seed_tic_rate client/npc/src` is
      nonzero.

## P3 — the headless clients see motion again

- [x] Replace npc's tile-only `pawns` map ([`lib.rs:266`](../../../client/npc/src/lib.rs)) with
      reads of core's track. Acceptance: no `HashMap<u32, (i32, i32)>` pawn store remains.
- [ ] Re-check the wolf chase against live subtile positions. Acceptance: a wolf closes on a
      moving bunny without overshoot across a 10-minute soak.
- [ ] Re-check bunny forage adjacency, which asks cheb ≤ 1 of a moving pawn. Acceptance: forage
      completions per minute do not drop against the P0 baseline.

## P3b — the intent queue is STATE, not a picture

- [x] Add `client/core/src/intents.rs` — entity → its `QueueState` entries, replaced whole per fan,
      deduped on `event_tic` with `codec::tic::tic_after`. Acceptance: unit test — a replayed older
      fan does not roll the snapshot back.
- [x] Settle the tic-ring comparison ONCE: delete `movers.rs:34`'s local `tic_newer` and call the
      codec (`tic.rs:41`). Acceptance: `grep -rn "fn tic_newer\|function ticAfter\|function
      ticNewer"` over `client/` is 0.
- [ ] Add the LOCAL OPTIMISTIC ARM — `arm_pending(entity, ref, at_tic, duration)` stamped when the
      host queues its own `EXECUTE_INTERACTION`, superseded by the next fan. Acceptance: unit test
      — busy at T+1 with no fan, idle past its tic deadline.
- [x] Add `busy(entity, now_tic)` — true for phase 2 until `fire_tic`, and for an unsuperseded arm
      until its deadline — plus `fires_at(entity)`. Acceptance: unit test covers both arms and
      `fires_at` is `None` for a phase-1 entry.
- [ ] Bound phase 1 with the mover track's own ETA (dest + pace) — `fan_program` gives a walking
      entry no end tic (`worker/src/main.rs:473-475`). Acceptance: unit test — a pawn whose walk
      stops advancing reads idle past its ETA, not frozen.
- [x] Clear the arm when a fan arrives with NO entry for that pawn — the REFUSAL signal brains
      cannot see today. Acceptance: unit test — arm, then an empty fan, `busy` false immediately.
- [x] Delete `busy_until` (`wolves.rs:65`, `bunnies.rs:35`) and both `dur / 6.0 + 3.0` conversions
      (`wolves.rs:409-412`, `bunnies.rs:365-369`) for `busy(id, now)`. Acceptance:
      `grep -rn "busy_until\|dur / 6.0" client/npc/src/brains` is 0.
- [ ] Replace the three wall-clock `deadline` watchdogs (`wolves.rs:929-937`, `bunnies.rs:201-211`,
      `debug.rs:116-125`) with the same track ETA. Acceptance: `grep -rn deadline
      client/npc/src/brains` is 0.

## P3c — one door: the input BINDER

- [ ] Add `bind_interaction(bundle, name, ctx)` to `shared/content/src/loader.rs` beside
      `interaction_params` (`:1681`). Acceptance: `attack`'s `["pawn","target","amount"]`
      (`interactions.toml:333`) binds in corpus order; a missing `target` refuses.
- [ ] Move `INTENT_FRESH`/`INTENT_COMPLETION`/`INTENT_ADVANCED` (`worker/src/main.rs:504-506`) into
      `shared/codec::action`, so nine packers stop writing a bare `0`. Acceptance:
      `grep -rn "const INTENT_FRESH" server/` is 0.
- [ ] Rewrite the three Rust binders as calls — `wolves.rs:579-613`, `bunnies.rs:215-235` and the
      SERVER's own copy at `worker/src/main.rs:1596-1615`. Acceptance: `grep -rn '"amount" =>'` over
      `server/worker/src client/npc/src` is 0.
- [ ] Expose it through `shared/wasm`, superseding `composeInteraction` (`lib.rs:62-72`), and point
      both webgl binders at it (`WorldScene.ts:957-976`, `:937-949`). Acceptance:
      `grep -rn "unbindable input" client/webgl/src` is 0.
- [ ] Delete the four hand-rolled positional programs — `wolves.rs:478-481`, `wolves.rs:712-714`,
      `bunnies.rs:194-196`, `debug.rs:108-110`. Acceptance: `grep -rn EXECUTE_INTERACTION
      client/npc/src` returns only the binder.
- [ ] Add `Command::Interact { interaction, binding }` to core, now that it owns the corpus.
      Acceptance: no host packs an action program — `grep -rn "EXECUTE_INTERACTION\|
      composeInteraction" client/webgl/src client/npc/src` is 0.

## P3d — one door: the AFFORDANCE query

- [ ] Add `affordance_eval::carrier_options` to `shared/content`, lifting
      `shared/wasm/src/lib.rs:1131-1161` minus the js_sys packing. Acceptance: unit test — an `on`
      bind refused at cheb 1, offered at cheb 0; a `destination` bind offered at cheb 7.
- [ ] Rewrite `menu_options` (`shared/wasm/src/lib.rs:1115`) as marshalling over it. Acceptance: the
      three `#[wasm_bindgen]` wrappers are untouched and `npm run build` is green.
- [ ] Replace npc's four hand-rolls (`wolves.rs:499-531`, `:543-573`, `bunnies.rs:239-254`,
      `:256-271`) with one call, keeping the `satisfy` narrowing as a predicate over the result.
      Acceptance: `grep -rn interaction_available client/npc/src` is 0.
- [ ] Route `mind_hunt` (`wolves.rs:442-497`) through `carrier_options` against the PREY's binds —
      today it fires `attack` ungated. Acceptance: a wolf whose `attack` tag is revoked issues ZERO
      attack intents (today three, then the ghost cap).
- [ ] Delete the hand-rolled shore picks (`wolves.rs:643-676`, `bunnies.rs:325-352`) — the worker's
      composer walks a destination-bearing order (`main.rs:2251-2281`). Acceptance: no cheb-ranked
      cell pick remains; drink completions/min hold vs P0.

## P4 — webgl becomes a display

- [x] Expose core's track to the browser through `shared/wasm` as `pawnPoint(entity, nowTic)`.
      Acceptance: at an anchor tic it equals the point that anchor's row carries.
- [ ] Point `MoverLayer`'s render-chase at `pawnPoint` instead of its own `Spec`. Acceptance:
      typecheck + build green and movers still glide between anchors.
- [x] Delete `Spec`, the walk, `speedFor`, `computePath` and `SPEC_APPLY_EPS` from `MoverLayer`.
      Acceptance: `grep -c 'walkGreedy\|walkPath\|speedFor\|computePath\|firstLegClear'` is 0.
      (Was "under 800 lines" — a proxy that the P0 probe's ~200 lines invalidated; 1266 today.)
- [ ] Move the P0 divergence probe out of `MoverLayer` into its own file, so the layer is
      sprite-sync only. Acceptance: `MoverLayer.ts` under 800 lines, probe still reports.
- [ ] Re-run the P0 probe for 10 minutes. Acceptance: reseed p50 under 0.5 tiles and zero RENDER
      teleport events — [I1](issues.md#i1) closed by construction, or reopened loudly.
- [x] Add a core test that feeds a `StateObject` through `Engine::emit` and reads `pawn_point` off
      the handle, covering the anchor-less `None` path. Acceptance: the test exists and passes.
- [x] Cover `next_hop`'s sub-1 `clear` CLAMP branch. Acceptance: a `panic!` inserted in
      `f.min(clear)` fails `a_partly_blocked_segment_lands_on_its_clear_prefix`.
- [ ] Resolve the RECENTER branch ([I10](issues.md#i10)) — reach it with real geometry, or delete
      it as dead. Acceptance: either a test that a `panic!` there fails, or the branch is gone.
- [x] Make `headless.rs` print one mover's point at two tics between anchors. Acceptance:
      `grep -c pawn_point client/core/src/bin/headless.rs` is nonzero and a run shows it moving.
- [x] Read a live pawn's thirst back through `Client::pawn_needs` and transcribe it. Acceptance: a
      nonzero value in `completed.md` — no code path produces this observation today.
- [x] Point webgl's `pawnNeeds` reader at the wasm accessor. Acceptance:
      `grep -c 'this.needRows' MoverLayer.ts` is 0 and a live thirst reads nonzero.
- [ ] Add `queueEntries` and `busy` to the wasm surface, with a webgl caller each. Acceptance:
      both exist and `grep -rn` finds a caller in `client/webgl/src`.
- [ ] Reconcile the two stale-intent rules: core rejects past a flat 16 tics, `MoverLayer.ts:837`
      scales with the destination span. Acceptance: both hosts reject the same intent set.
- [ ] Delete the worker's private hop tic-cost re-derivation (`main.rs:1662-1675`) — it applies
      `hop_stride_tiles` without the clamp and floors at 4.0, so `k` can disagree with `Hop::tics`.
      Acceptance: the scheduler calls `move_eval`.
- [ ] Run the foreground soak P4 keeps deferring. Acceptance: a dated row naming duration, sample
      count and mover population, with reseed p50/p90 and RENDER teleports per MINUTE.
- [ ] Grow the wasm read surface ONCE — `WorldClient` (`shared/wasm/src/lib.rs:1226`) is write-only
      today. Add `nowTic`, `pawnPayload`, `pawnNeeds`, `queueEntries`, `busy` beside `pawnPoint`.
      Acceptance: `npm run typecheck` + `npm run build` green.
- [x] Return needs as stride-2 `Float64Array`, NEVER `Uint32Array` — the documented truncation at
      `MoverLayer.ts:805-812` cut `0xa4fb80010020` to `0x80010020` and every need read 0.
      Acceptance: a live pawn's thirst reads nonzero through the accessor.
- [ ] Collapse webgl's eight hand-rolled u16 now-tic conversions onto `nowTic()` with ONE null
      policy — bail. Acceptance: `grep -rn "0x10000) + 0x10000" client/webgl/src` is 0, killing
      `now = 0` at `WorldScene.ts:298,:359` and `MoverLayer.ts:858`.
- [x] Point `MoverLayer`'s `payloads`/`needRows` (`:382`, `:385`) and every `WorldScene` reader
      (`:158-159`, `:284`, `:296`, `:879-894`, `:933-934`) at the accessors. Acceptance:
      `grep -c "this.payloads\|this.needRows" MoverLayer.ts` is 0.
- [ ] Reduce `IntentQueues.ts` to change notification over core's queue track. Acceptance: the file
      holds no entries map and `IntentStrip.ts` still draws the ring from `started_tic`/`fire_tic`.

## P5 — the guard, so it cannot come back

- [ ] Add a `rd docs-check` rule failing on a simulation constant or stepping rule defined outside
      `shared/`. Acceptance: it fires on a deliberately reintroduced copy, green otherwise.
- [ ] Write the simulation-vs-presentation line into `docs/components/client/webgl/intent/`:
      shared answers move, webgl only smooths them. Acceptance: docs-check link integrity green.
- [ ] Extend the docs-check rule with a second case: an AFFORDANCE decision or an input BINDING
      composed outside `shared/`. Acceptance: it fires on a reintroduced
      `match name.as_str() { "pawn" => … }` in a host, green otherwise.
- [ ] Add a third case: a wall-clock `Instant::now()` / `Date.now()` used as a SIMULATION deadline
      outside `client/core`. Acceptance: it fires on a reintroduced `busy_until: Option<Instant>`.
- [ ] Re-label `api.rs:254-263` — `QueueState` is an authoritative, self-correcting, NON-DURABLE
      snapshot (the worker's ephemeral map stays authority, lumberjack F1), not "Display truth
      only". Acceptance: the phrase is gone; docs-check green.
- [ ] Re-scope `ACTIONS.md:308`'s "Presentation only" to "not an authoritative WRITE", and restate
      its "never a wrong action" law as conditional on core's local optimistic floor. Acceptance:
      docs-check green.
- [ ] Amend [`client/core/intent/client.md`](../../components/client/core/intent/client.md): the one
      contract is commands IN, events OUT **and answers on request** — the read surface is part of
      it. Acceptance: docs-check link integrity green.

## P6 — the exit

- [ ] Confirm [I2](issues.md#i2)'s pre-fan fallback is gone — core knows the pace before it moves
      anything. Acceptance: no mover ever reports `DEFAULT_TICS_PER_TILE` during a cold load.
- [ ] User watches the fluffle at `:5174/?user=Claude&focus=124,75&zoom=1`. Acceptance: no bunny
      visibly jumps.
- [ ] Write the exit record and the memory line. Acceptance: docs-check green.
- [ ] Confirm no host folds a fanned row into its own model. Acceptance: `client/npc` holds no
      `HashMap<u32, …>` entity store and `client/webgl` holds no `Map<number, …>` entity store of a
      fanned row.
- [ ] Run a 10-minute soak with **no browser open**, answering position, rows, busy and now-tic from
      core alone. Acceptance: eat and drink completions per minute hold against the P0 baseline.
