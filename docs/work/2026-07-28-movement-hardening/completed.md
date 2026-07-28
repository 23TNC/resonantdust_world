# Completed — movement-hardening

_Dated entries, appended as items land: what landed and how it was verified._

## 2026-07-28 · P1 — chain supersession (6/6)

**Docs**: `MOVE_STEP` palette row (value 8, arity 3, worker-only), §Movement chain-identity
paragraph, `TABLES.md` § pawn `data` bit table; two refinements ratified during build —
every `MOVE_TO` is a seed (the verb split obsoletes `PROMOTE_EVENT` sniffing) and the serial
comes from `event_reference & 0x3F` (unique even for two SAME-TIC intents, where a
tic-derived serial would let both chains live); the stale `init_zone *tbd*` palette row fixed
in passing (built, value 7). **codec**: `MOVE_STEP` signature/sets/Hot-routing +
`pack_pawn_data`/`pawn_trip_serial` riding the existing `rotation|count` bit split; 54 tests
green incl. framing/routing and pack round-trip; wasm32 build green. **worker**: `MOVE_TO`
arm = pure seed (stamp serial + facing, step nothing), new `MOVE_STEP` arm dies on serial
mismatch with one debug line, CONTINUE queues `MOVE_STEP obj dest serial` (PROMOTE on the
landing hop) and skips superseded chains. **edge**: `CLIENT_VERBS` allowlist at the queue
door. **npc**: deadline re-issue supersedes with a FRESH dest.

**Verified live**: MOVE_STEP chains hop at exactly 12-tic spacing (5497→5581 logged), trips
arrive at the `hops+1` cadence (7 hops ≈ 15 s), CREATE→spawn→adopt clean. **Allowlist**: a
browser-queued raw `[8, obj, dest, serial]` never reached the shard (polled `event_log`
while allowed traffic flowed on the same connection); the QueueErr reply precedes any
reducer call by construction (not directly observed — the webgl host doesn't render status
text). **Supersession drill**: a second session's `MOVE_TO` hijack mid-12-hop-trip killed
the npc's chain at its NEXT hop (`chain superseded — hop dies serial=45 stamped=54`,
exactly one line); the npc's deadline then superseded BACK with a fresh trip from the
authoritative position; queue depth stayed 1–2 throughout; steady-state trips resumed at
exact cadence. BONUS: the mechanism also reaps the natural trip-boundary race (a stale
final hop dying after the next trip's seed — observed twice, one line each).

**Three landmines found + handled on the way** (see issues.md): I1 module hashes missed
`shared/codec` (fixed — a codec change now republishes every module); I2 ghost adoption
after a shard wipe (P2 gains the npc item; the ghost re-materializes at position 0 via
default-payload compose); I3 orphaned dirty claims wedge composition forever (fixed:
`ABANDON_TICS = 64` — the worker bases past ancient dirty slots on the latest clean row,
and the macro `gc` reaps a dirty row once a newer clean row supersedes it; a pending claim
with no newer clean row is never reaped, so a slow worker loses nothing).

## 2026-07-28 · P2 — phantom StateGone (2/2, + the I2 npc item)

Edge: per-connection `entity → (zone, seq)` tracker; deletes route through `relay_gone` — a
delete for an entity live in another zone is swallowed outright, otherwise a 400 ms hold
relays only if NO reappearance lands (`seq` catches even same-zone delete+reinsert churn;
callbacks spawn the hold on a captured runtime handle — the SDK thread has no tokio context).
**Drilled both ways**: 11 cross-zone trips with the wander disc straddling a zone-column AND
zone-row boundary — ZERO phantom removals at the browser (removal hook, 2.6 min) and ZERO
artifact lines at the npc (debug level); a hand-DELETEd pawn row relayed `StateGone` after
the hold. I2 closed on top: `StateGone` being trustworthy, the wolves brain now HONORS
removals — drilled: delete → "dropping adoption" → 3 s adopt-window → CREATE → new wolf
`0x30800003` adopted, in one container lifetime.

## 2026-07-28 · P2.5 — render-chase (user design, 1/1)

The rendered position is its own track chasing the speculated one: Chebyshev per-axis
stepping (matches the greedy path's diagonal geometry), catch-up = `min(1.2, 1 + gap·0.5)` ×
true speed (learned `ticsPerSec` / authored tics-per-tile), snap past 3 tiles, dt-clamped;
authoritative rows steer targets only (no direct paint for existing movers); target facing
applies immediately (the seed's turn-toward-path). Two bugs found by measurement on the way:
(1) gating the chase STATE on the bake eps froze the render until the snap threshold —
sub-eps steps must ACCUMULATE (split `rx/ry` state from `arx/ary` last-baked); (2) the
estimator's ahead-poison band locked fresh pages at d = −4600 when a DEAD wolf's resting row
(legitimately ancient tic) anchored first — fixed with a symmetric ahead-streak self-heal
(6/6 ticclock tests). **Measured on a visible tab**: 235/295 100 ms windows in continuous
motion, arm-lag (d ≈ 4.9) closed smoothly, cap held in the math (the 0.62-vs-0.54 peak is
1/32-tile bake quantization aliasing across window boundaries). Debug globals `__movers` /
`__moverLayer` added alongside `__client`/`__content`.

## 2026-07-28 · P3 — master pacing (3/3): the 6 Hz promise kept

**Diagnosis chain (instrument, don't guess — each stage exonerated by numbers)**: the loop
itself is metronomically perfect (361 iters/min, mean wait 166.1 ms, body 0.1 ms); every
`bump_tic` is SERVER-CONFIRMED (`_then` callbacks: 360/360 per window, 0 failures) — yet the
durable counter gained ~322/min. External-truth checks: container realtime == host realtime
(33347 vs 33348 ms over one window) and host realtime ≈ internet time (±1 s HTTP-Date noise),
but **`sleep 30` takes 33.35 REAL seconds — CLOCK_MONOTONIC runs at 0.900× realtime in this
WSL2 stack**, in the host distro AND the docker VM. 6 Hz × 0.90 = the measured 5.40. The
authored clock was never wrong; every monotonic sleep in the VM is 11% long (env note: this
skews EVERYTHING that sleeps — npc deadlines run long (harmless slack), docker healthchecks…).

**Fix**: realtime (`SystemTime`) grid + an INTEGRAL controller on the measured achieved-Hz
(per-tick feedback alone measurably did not converge here — steady 5.44; wherever the residual
dilation hides, the achieved-rate signal sees it): each ~360-tick window scales the effective
period by `sqrt(achieved/target)` (half-gain — one noisy window nudges, not yanks), clamped;
falling mildly behind ticks at ≤ 1.2× rate (spacing ≥ ⅚ period — the no-sleep catch-up
iterations were measured micro-bursting a window to 6.3 Hz, F4's violation in miniature);
> 2 periods behind snaps the grid forward, never bursts. **Measured**: convergence
5.427 → 5.689 → 5.829 → 5.918 → …; durable **5.9986 Hz realtime over 157 s** (target ≥ 5.95).
Honest residual: the controller wobbles ±5% between settling windows, so momentary wall speed
breathes slightly around 2 s/tile; the amplitude shrinks as windows accumulate.

**Client re-measure at the fixed clock** (fresh page, 11 trips): learned rate 6.12, arms `d`
flat 4.0–5.3 (single-digit, no climb), landings e ≤ 0.26 except the first trip's e = 1.1 —
the rate-learning warmup, which P4's rate seed exists to kill.

*Commit note*: a concurrently-active session (art-128-tiles) ran `git add -A` mid-P1, so this
phase's CODE landed inside its commits `ca98cf0` + `6684ab5` (art-titled messages). Nothing
lost — verified file-by-file — but attribution interleaves; this session stages selectively
from here on while both are live.
