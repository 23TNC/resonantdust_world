# Completed — sim-self-heal

## 2026-07-28 · P1 — `server/uplink` (`resonantdust-uplink`)

`Uplink<C, S>`: closure-built (F3 — no SDK types named, unit-testable with a fake `C`),
`get()`-only API returning the live conn or rebuilding it, subscribe-and-wait gating (a conn
that dies DURING subscribe is also rejected), capped doubling backoff 0.5 s → 8 s with
attempt-count warn logs and a "reconnected after N attempts" info. The `alive`-from-three-
places rule (disconnect + connect-error + subscription on_error) is the caller's build-closure
contract, documented at the top of the crate. Verified: 4/4 unit tests green in docker
(recover-after-down, dead-flag-triggers-exactly-one-rebuild, get-waits-for-subscribe,
backoff-schedule-with-cap under tokio's paused clock — which found a real bug: the backoff
originally used `std::time::Instant`, invisible to tokio's virtual clock; now `tokio::time::
Instant`). NOTE: the plan's acceptance said `bin/rd check` — no such command exists; the
equivalent gate used is in-docker `cargo test`/`cargo check` via the sim builder image.

## 2026-07-28 · P2 — master converted + three live drills

All 6 connections are `Uplink`s (`shard_uplink!` macro stamps the build closure with `alive`
wired into connect-error + disconnect; the index uplink carries the `master_clock`
subscribe-and-wait with sub `on_error` clearing the SAME flag). The loop `acquire`s per
upstream and logs only up/down TRANSITIONS (the uplink's backoff paces its own warns);
`set_orchestrator` re-stamps per event-shard connection GENERATION (a republished shard
forgets it — the standup-only call was itself a latent bug). A dead index skips the pass;
a dead shard skips only its own bump/sweep.

Drills, all verified live with zero master restarts:
1. **All-up:** tic 31398→31411 advancing, orchestrator stamped via the generation path.
2. **Partial outage:** `spacetime delete …-pawn-0` mid-run → ONE "uplink down — skipping its
   work" warn, tile clock kept ticking (31566→31579); republish → "uplink up — resuming",
   fresh pawn clock immediately riding the metronome (31762→31775). This is the first-pawns
   I1 void-write, cured for the master.
3. **Cold start:** master started with the DAEMON stopped → 6 paced retry warns, no panic;
   daemon started → all uplinks up within seconds, tic resumed from the durable row
   (31906→31919), container "Up 21 seconds" — never restarted.

## 2026-07-28 · P3 — worker + orchestrator converted; chaos + republish drills pass

The wiring macros moved INTO the uplink crate (`uplink!` sub-less, `subbed_uplink!` with
subscribe-and-wait + sub-`on_error` clearing the same flag, `acquire()` transition-logger) —
one implementation for all three binaries; the master refactored onto them. Worker: all 6
connections are subscribed/plain uplinks; the pass acquires everything up front and DEFERS on
any dead upstream (events stay ASSIGNED; writes absolute + replay-safe). Orchestrator: same
shape (its per-component claim failures were already tolerant). Both re-checked with in-
container `touch` + `cargo check` (the host-side touch did NOT defeat the docker mtime miss —
the P5 guard item just earned its keep).

Drills, verified live:
- **Chaos (daemon restart mid-run):** all three logged the 6 disconnects at 14:54:36 and
  reconnected by 14:54:37 — so fast the "uplink down" transition never printed — with zero
  container restarts (uptimes unchanged); tic advancing (34066→34079). The un-converted EDGE
  died with the daemon both times (its `.expect` disease — the wolf stalls until the edge is
  redeployed), which is exactly the P4/P5 territory.
- **Republish (the first-pawns I1 void-write):** `pawn` republished mid-run → the worker's
  pawn uplink dropped + rebuilt in 24 ms and the FRESH DB immediately received composed rows
  (mint 0x30800000 at tic 34388) — no restarts, the void-write class is dead for the trio.
- Bonus observation: after the earlier outage the worker drained its backlog of stale ASSIGNED
  events on reconnect — replay-safety doing real work.

## 2026-07-28 · P4 — the client side heals

**Engine auto-reconnect** (`client/core/src/engine.rs`): an unplanned disconnect (socket
close/error) schedules a re-login as the remembered session (doubling backoff 0.5 s → 8 s);
the full flow re-runs (gateway resolve → connect → login), and on success the engine REPLAYS
the anchors snapshotted at disconnect (subscriptions are per-session; anchors are the durable
intent — `ZoneManager::anchors()` added). `Command::Logout` forgets the session so a planned
exit never resurrects. Failed re-logins (gateway 503, WS refused, LoginErr) reschedule.

**Drill, verified live:** killed the edge PROCESS under a running npc → "disconnected …
Connection reset" → visible backoff retries (1 s, 1 s, 2 s, 4 s…) → edge restored → "logged in
player_id=1025" and the trip the brain had been re-issuing during the outage ARRIVED seconds
later (anchor replay proven — subscriptions only exist via anchors); `rd-npc` uptime
unbroken. The ZOMBIE npc class is dead — by healing, not exiting.

**Item 2 resolved as its "(or besides)" arm:** with reconnect built, exiting on mid-run
`Disconnected` would be a regression — the brain now SURVIVES. The no-zombie intent is
delivered by the heal; startup login failure still exits 1 so the restart policy retries.

**Restart policy:** `bin/sim run` starts every `rd-*` container `--restart=on-failure:10`
(verified via docker inspect: `on-failure`); the npc exits non-zero on a failed startup login,
so "edge not up yet" self-resolves.

## 2026-07-28 · P4b — edge lock poison

`server/edge/src/lock.rs`: one `RwRecover` trait (`read_r`/`write_r` =
`unwrap_or_else(|e| e.into_inner())` + a warn) — sound here because every locked value is
swapped/updated whole (snapshot Arcs, version stamps), so a poisoned guard still holds the
last consistent value. All 12 sites across `content.rs`/`connections.rs`/`tex_manifest.rs`
converted; grep finds 0 `.read().unwrap()`/`.write().unwrap()` in the edge; build green.
Deployed live with the P5 verification redeploys.

## 2026-07-28 · P5 — dev-redeploy hardening (with two plan corrections)

**Correction 1:** `target/` was ALREADY excluded from module hashes — the real data-wiping
trigger was the in-container build writing `Cargo.lock` AFTER the pre-action hash was stamped.
Fixed by RECOMPUTING the stamp after the action (build-generated files join the clean
baseline; a hand-edited Cargo.lock still redeploys). Verified: force-republish `pawn` → the
very next `rd redeploy --run` says "nothing changed — up to date".

**Correction 2 (F4):** the "bounce consumers" items are superseded by the self-heal the earlier
phases built — the P3 drill already proved a mid-run republish lands the next compose with
zero manual steps. Implemented as status lines instead: module publish → "live sim processes
self-heal in place: rd-npc rd-worker rd-orchestrator rd-master" (observed live); edge deploy →
a browser-reload reminder.

**`bin/sim` guards:** `cmd_run` refuses a binary older than any source under the crate's
path-dep roots (per-crate `dep_roots`; `FORCE=1` overrides) — verified: touch worker's main.rs
→ run refuses with the exact file named; build → run proceeds. `cmd_build` fails loudly on a
cargo error and warns when sources were stale yet cargo compiled nothing (the mtime-miss
signature). The warn's NEGATIVE path is live-verified (touch → build → "Compiling worker" → no
warn); the POSITIVE arm is verified by construction only — every simulation attempt (backdated
binary etc.) made cargo correctly recompile, which is cargo working, not the guard failing.

## 2026-07-28 · P6 — wrap; STREAM COMPLETE (21/21)

Final health check after a session of deliberate chaos (daemon restart, shard delete +
republish, edge process kill, module republishes): all four `rd-*` containers up with
unbroken uptimes, the wolf tripping continuously (6 trip lines in a 15 s window), tic
advancing 37681→37694. Evidence for every drill is in the phase entries above with the
observed log lines. **Out-of-scope follow-up:** migrate `server/gateway/src/directory.rs`
onto `resonantdust-uplink` (it still runs its own copy of the pattern, minus the
sub-`on_error` leg and backoff); also the edge's per-client shard connections could ride
uplinks one day (today a dead per-client upstream surfaces as an error frame and the client
reconnects — the engine heal covers it end-to-end).
