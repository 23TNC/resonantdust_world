# Todo — movement-hardening

_Items tick in place; the box is the move. Design: [`README`](README.md) ·
[`ACTIONS.md`](../../ACTIONS.md) §Movement · [`TABLES.md`](../../TABLES.md) § pawn ·
[`forks`](forks.md)._

---

## P1 · Chain supersession — at most one live chain per pawn (pawn-movement I7)

- [x] Docs FIRST: `ACTIONS.md` — `MOVE_STEP` §World palette row (value 8, arity 3:
      `obj` **write**+**read** · `dest` imm · `serial` imm; WORKER-ONLY — never valid from a
      client) and §Movement rewritten for chain identity (seed stamps the serial, hops carry
      it, mismatch dies silently, new intent supersedes by construction, re-issue is safe).
      `TABLES.md` § pawn — the `data` u8 bit layout (bits 6–7 facing, bits 0–5 trip-serial).
      Acceptance: `bin/rd docs-check` green.
- [x] codec: `MOVE_STEP = 8` (signature, write/read sets, `target_routes` Hot — mirror
      `MOVE_TO`), plus `data` pack/unpack helpers (`facing`, `trip_serial`, compose) so the
      worker and any future reader share one layout. Unit tests: framing, sets, routing,
      pack round-trip. Acceptance: codec tests green (native + wasm32 build).
- [x] worker: the intent seed stamps `data = facing<<6 | (event_tic & 0x3F)` and queues
      `MOVE_STEP obj dest serial`; the `MOVE_STEP` arm checks the pawn's stored serial —
      match ⇒ step+face+re-queue (PROMOTE on the landing hop, exactly as today), mismatch ⇒
      chain ends (one `debug!` line, no step, no re-queue). `MOVE_TO` keeps ONLY its seed
      role. Acceptance: soak shows trips unchanged (intent + seed + landing, hops at
      `tics_per_tile` spacing) with `MOVE_STEP` continuations in the queue.
- [x] edge: client-program verb ALLOWLIST at the queue door (reject `MOVE_STEP` and any
      server-only verb with a clear error; `PROMOTE`/`PROMOTE_EVENT`/`CREATE`/`PLACE`/
      `MOVE_TO`/`SET` stay). Acceptance: a hand-queued `MOVE_STEP` from a client session is
      rejected; the npc soak is unaffected.
- [x] npc: deadline re-issue is now SAFE (new seed = new serial = old chain dies at its next
      hop) — re-issue picks a FRESH dest from the current position instead of repeating the
      lost one; comment updated to the supersession contract. Acceptance: forced-deadline
      drill (temporarily slash the slack) shows re-issues with NO tug-of-war: single winner,
      no per-hop promotes, landing error small.
- [x] Supersession drill: mid-trip, queue a SECOND move for the same wolf to a different dest
      (second bot session or a temporary npc hook). Acceptance: worker logs exactly one
      superseded-chain line; the wolf walks only the new path; browser sees 1 intent + seed +
      landing per WINNING chain and no per-hop promotes; `event_log` holds no orphaned
      continuations after the trip.

## P2 · Phantom StateGone on zone migration (first-pawns I4)

- [x] edge: per-connection entity→zone tracker in the relay — a delete for an entity live in
      a DIFFERENT subscribed zone is swallowed (migration); a delete matching the entity's
      current zone holds one beat (~2 tics) and relays `StateGone` only if no insert lands.
      Component doc updated (`docs/components/server/edge/`). Acceptance: cross-zone drill —
      wolf wanders across a zone boundary with the browser watching: NO `StateGone` relay,
      no warm-prim drop/re-add; the npc's "StateGone (zone-migration artifact)" debug line
      stops appearing.
- [x] Real-removal path still works: delete the pawn's row by hand (spacetime sql / a gc
      pass) and confirm `StateGone` relays after the hold; downgrade the wolves-brain comment
      from "workaround" to "defense" (adoption-keep stays). Acceptance: drill evidence in
      `completed.md`. (Went further than planned — I2: with `StateGone` now trustworthy the
      npc HONORS removals: drop adoption → adopt-or-CREATE window → fresh mint.)

## P2.5 · Render-chase (USER DESIGN, added during execution — the tween knob un-held)

- [x] MoverLayer: the RENDERED position chases the SPECULATED position in path-progress
      space — catch-up capped at +20% of the pawn's speed (non-linear: larger gap → harder
      chase within the cap), snap only past a hopeless threshold; authoritative rows keep
      steering the target; the contract is agreement at the destination + broad agreement
      along the path. Kills the arm-time forward snap (~0.4 tiles of settle+fan latency) and
      every small correction snap. `ACTIONS.md` §Movement knob text updated to BUILT.
      Acceptance: browser soak — trips open with NO visible snap (chase from the start tile),
      landings still e ≈ 0 in SPEC space, and the render never exceeds 1.2× authored speed.

## P3 · Master pacing — keep the 6 Hz promise (pawn-movement I5)

- [x] Instrument, don't guess: per-stage timing in the master loop (interval overshoot,
      `bump_tic` call latency, dedup-skip counts) logged as a 1/min drift summary.
      Acceptance: the lagging stage is IDENTIFIED with numbers in `completed.md`.
- [x] Fix the measured stage under F4's constraint (no burst catch-up — hold the PERIOD; a
      tic is a wall-time promise). Acceptance: durable tic ≥ 5.95 Hz over a ≥ 5-min sql
      sample in the dev container; the browser's learned rate reads ≈ 6.0; a wolf tile takes
      ≈ 2 s wall.
- [x] Re-measure the estimator against the fixed clock (it must stay correct when rate ≈
      authored): arms `d` single-digit, landings e ≤ 0.3 from a fresh page. Acceptance:
      soak numbers in `completed.md`.

## P4 · Client clock polish + robustness follow-ups

- [x] Rate seed: persist the last learned `tics_per_sec` in the webgl host (localStorage,
      keyed by gateway URL, written on each `ticAnchor`) and seed the engine estimator at
      boot via a small wasm export — a clamped HINT (F5), stream stays authoritative.
      Acceptance: fresh reload's FIRST trip lands e < 0.3 (today ≈ 1 during warmup).
- [x] gateway: migrate `directory.rs` onto `resonantdust-uplink` (the pattern's birthplace —
      last unconverted surface, 4 panic sites). Acceptance: login works; kill/restart
      SpacetimeDB mid-run → gateway heals without a process restart (the sim-self-heal drill,
      run against the gateway).
- [x] Docs hygiene: re-verify `docs/components/server/spacetime/modules/index/current/`
      against the module (stale-stamp warning since 07-17) and re-stamp. Acceptance:
      docs-check no longer warns about it.
- [x] Wrap: memory updated (supersession contract, 6 Hz restored, StateGone clean),
      pawn-movement I5/I7 + first-pawns I4 stamped closed with pointers here, work-index row
      → done.
