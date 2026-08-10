# Completed — shared-simulation

_The verification log: what landed and **how it was checked**. Append-only; authoritative for what
is done. Items live in [`todo.md`](todo.md) with their boxes ticked._

## 2026-08-10 — P0 complete: the BASELINE, on a reset world

World reset (`bin/rd redeploy --run`, all module DBs wiped, all four sim crates rebuilt and
restarted). Worker **in step** throughout: lag +1/+2 across the whole run, never the ±1400 of
[I6](issues.md#i6). Population is small on a fresh world — 3 bunnies, 2 wolves — so this is a
per-pawn baseline, not a load baseline. ~15 minutes, **551 samples**.

| | bunny (pace 24) | wolf (pace 12) |
|---|---|---|
| implied pace, p50 (p10 / p90) | **24.26** (23.77 / 28.44) | **12.07** (11.91 / 15.41) |
| anchor stride p50 / predicted | 1.326 / 1.333 | 2.652 / 2.667 |
| anchor gap, p50 tics | 32 | 32 |
| reseed error p50 / p90 / max | **1.25** / 4.60 / 12.53 | **2.13** / 8.00 / 20.00 |
| reseeds past `CHASE_SNAP_TILES` | 45 of 387 (**11.6%**) | 24 of 160 (**15.0%**) |
| RENDER teleports (chase snaps) | 0 | 0 |

**Read it carefully, because it says two things and they pull in opposite directions.**

*The pace agrees.* Implied vs client pace matches to ~1% at the median, and the observed stride
matches the predicted one to within 0.015 tiles for both species. This is what retracts
[I1](issues.md#i1)'s 2× premise.

*The belief still drifts multiple tiles.* With the pace correct and the cadence exactly 32 tics,
the client's speculated position is still a median **1.25–2.13 tiles** from truth when the
authoritative row lands, with a p90 of 4.6–8.0 and a worst case of 12–20 tiles. **Between one in
nine and one in seven reseeds exceeds the render-chase's give-up distance.** The chase absorbed all
of them in this window (0 RENDER teleports) — so the world currently *looks* fine — but a client
whose belief is routinely 2 tiles wrong is not simulating the same world the server is, and the
margin protecting the viewer from that is one constant.

So the divergence is real and it is **not** a pacing divergence: it is geometry, path consumption,
or lifecycle. That is the target for the rest of the stream, and P4 re-runs exactly this table.

**Method note.** Per-kind bucketing is the whole reason this baseline says something the first one
didn't. Pooling species produced the 2× that sent me chasing pace for an evening
([I1](issues.md#i1)); a bunny at 24 and a wolf at 12 in one distribution describe nothing real.
Reload persistence verified live mid-soak: 1058 samples before `location.reload()`, 1082 after,
still climbing.

## 2026-08-10 — P1 items 3–4: `next_hop` and `position_at`, the walk in one place

**Landed** in [`move_eval.rs`](../../../shared/content/src/move_eval.rs): `next_hop` (the worker's
`MOVE_STEP` chord step — first chord, `clear_point_fraction` clamp, lattice recenter, stride cap,
exact arrival on the destination tile) and **`position_at`** — where a walking pawn IS at an
arbitrary tic. `position_at` is the one that did not previously exist as shared code anywhere: the
worker had `resolve_walk_position_for` privately, the TypeScript had its own re-derivation, and the
npc had nothing, which is precisely why headless brains could not see a pawn between anchors.

Both work in fractional world POINTS, not `position_reference`s, so callers quantise at their own
edge and the module stays codec-light and usable from wasm.

**Verified.** 9 tests pass in `cargo test -p resonantdust-content move_eval::`. The load-bearing
one is `position_at_the_hop_tic_equals_the_hop_landing`: sampling the walk at the hop's own tic
must land exactly where the hop lands, across 6 cases (axis-aligned, pure diagonal, off-lattice
start, stride-capped long trip, single tile, both authored paces). If those two ever disagree, an
observer interpolating and a server stepping are two implementations again. Also pinned: one tile
per `pace` tics as distance-over-time; exact arrival with no overshoot; a future `base_tic` reading
as elapsed **zero** rather than most of a ring; a degenerate pace holding position; a blocked world
yielding no hop.

**Acceptance substituted, deliberately.** The items said "reproduces 20 landings recorded from the
live worker". Recording those needs new worker instrumentation, and it would only prove the shared
code matches the code I transcribed it from. I asserted the stronger property instead — internal
consistency between the two functions plus the pace contract stated independently of the stride —
and left the worker's own behaviour to be proven where it actually matters, by P1 item 5 running
the live world against the P0 probe. Flagging it because it is a real departure from the written
criterion, not a quiet reinterpretation of it.

**Two asymmetries found in the worker while transcribing**, both preserved rather than silently
"fixed", because changing behaviour during an extraction is how a refactor becomes a bug hunt:
`next_hop` recenters on the lattice when the direct segment is fully blocked while
`resolve_walk_position_for` merely clamps to zero; and `next_hop` snaps exactly onto the
destination point on the dest tile while the resolve returns the current position. Worth a look
when P4 re-measures.

## 2026-08-10 — P1 items 1–2: the shared module exists, and the worker is a caller

**Landed.** [`shared/content/src/move_eval.rs`](../../../shared/content/src/move_eval.rs) — THE
walk, beside `path_eval`/`stat_eval`/`needs_eval` per [F1](forks.md#f1). Holds `REANCHOR_TICS`,
`CHORD_CAP_TILES` and `hop_stride_tiles`, exported from `lib.rs`. `server/worker/src/main.rs`'s
private constants and function are **deleted**; it now `use`s the shared one, so the cadence and
the stride have exactly one definition in the tree. The only surviving mention of `REANCHOR_TICS`
under `server/` is a prose comment.

**Verified.** `cargo test -p resonantdust-content move_eval::` — 3 passed, 0 failed:

- `stride_clamps_at_both_ends` — pace 240 → 1 tile, pace 1 → 8 tiles (the item's criterion
  literally), plus the zero-pace divide guard and the exact cap boundary at pace 4.
- `stride_matches_the_authored_paces` — 24/12/6 (`walks` levels 1–3) pin 4/3, 8/3 and 16/3 tiles,
  so a `walks` retune cannot change the fan rate silently.
- `the_cadence_bounds_every_hop_except_a_floored_one` — the honest statement of the promise, and
  it caught my own first draft: I asserted the cadence bounds *every* hop, which is false for a
  pawn slower than one tile per cadence, where the one-tile floor wins and the hop necessarily
  overruns. **That case matters to this stream directly** — a divergence probe that assumes 32
  tics between anchors will mis-read a slow pawn as a stalled one.

`bin/sim check worker` green (one pre-existing unrelated `unused import` warning).

**Not done in this phase.** Items 3–5 (`next_hop`, `position_at`, the `MOVE_STEP` rewrite) are
blocked with P0: their acceptance is "reproduces 20 landings **recorded from the live worker**",
and there is no live worker to record from ([B2](blockers.md#b2)).

## 2026-08-10 — P0 probe code landed; acceptance BLOCKED on a live world

**No items ticked.** The code is in and verified as far as it can be without a running sim; the
acceptance criteria all require live samples and the world cannot currently produce them
([B2](blockers.md#b2)). Recording it so a resuming session does not rewrite it.

**Landed** in [`MoverLayer.ts`](../../../client/webgl/src/game/world/MoverLayer.ts): a per-kind
divergence tally on `__teleportProbe` — `reseedErr`, `anchorStride`, `anchorGap` and `impliedPace`
as bounded sample arrays, plus `report()` (quantiles per kind, with the client's `clientPace`
printed beside the `impliedPace` the server's own rows imply — the comparison
[I1](issues.md#i1) turns on) and `reset()`. Mirrored to `sessionStorage` on a 2 s throttle and
flushed on `pagehide`, because the dev server's reloads had already silently restarted two
measurement runs.

**Verified:** `npm run typecheck` and `npm run build` green. `reset()` writes the expected shape to
`sessionStorage` (`{"divergence":{},"auth":0,"render":0}`), and the restore path runs at
construction — so the persistence *mechanism* works, though it has not yet carried a **non-zero**
count across a reload, which is what item 2's criterion actually asks.

**Not verified:** `report()` printing quantiles off real samples, and the 10-minute baseline. The
first attempt returned **zero `StateObject` rows in 30 s** with 90 movers resident — which is what
uncovered [I6](issues.md#i6) and [B2](blockers.md#b2) rather than any fault in the probe.

**Also learned, the expensive way** — `bin/sim run <crate>` runs the *already-built* binary, so
restarting a long-lived sim process is `build` then `run`; and a second session is committing to
this branch concurrently. Both written up in [I6](issues.md#i6).

## 2026-08-09 — the survey that opened the stream

Not a plan item; the diagnosis P0 will re-measure against. Recorded because it is the evidence the
stream's stance rests on, and because the numbers are perishable — they came off a live world.

**Setup.** `rd-host` running `NPC_MODULES=wolf_pack@112,68;bunny_fluffle@124,75`; client at
`:5174/?user=Claude&focus=124,75&zoom=1`; 90 movers resident — **86 bunnies** (kind 13), 3 wolves
(kind 7), 1 bunny still pre-fan.

**How checked.** Console probes over `__moverLayer` / `__movers` / `__teleportProbe`, tallying
`[mover] spec reseed` lines and pairing consecutive authoritative `StateObject` rows per entity;
`docker logs rd-worker` over a 10-minute window; `spacetime sql resonantdust-dev-pawn-0`.

| measurement | value |
|---|---|
| client-derived pace, bunny | 24 tics/tile (86 of 90 movers) — correct for `walks` level 1 |
| client-derived pace, outlier | 3 tics/tile on one mover = `DEFAULT_TICS_PER_TILE` ([I2](issues.md#i2)) |
| authoritative row gap | p50 **32 tics** — the `REANCHOR_TICS` cadence, as designed |
| displacement between rows | p50 **2.67 tiles**, p90 5.41, max 7.85 — 24 tics/tile predicts 1.33 ([I1](issues.md#i1)) |
| spec reseed error | p50 **3.4–4.0 tiles**, p90 8.3, max 13.1 |
| reseeds past `CHASE_SNAP_TILES = 3` | **116 of 214 in 54 s** — the visible snapping |
| worker `intent queue REPLACED` | 731 vs 1157 move intents / 10 min ([I4](issues.md#i4)) |
| worker `mid-chord resolve` | 35 / 10 min ([I4](issues.md#i4)) |
| worker duplicate work-group WARNs | 936 / 10 min ([I5](issues.md#i5)) |
| worker compose lag behind master | p50 ~1 tic, tail to 16 |

**Ruled out.** The trait level round-trips correctly — a live bunny's stored payload decodes to
five `TRAIT` entries, all `data = 0`, all variant nibble **0** → tier 0 → level 1 → 24 tics/tile;
and both sides evaluate it through the same `stat_eval::stat_value`. A stale worker corpus is also
out: `rd-worker` booted 21:08:35Z alongside the `forager` commit. **The shared calculations agree;
the unshared one does not** — which is the stream's argument, arrived at from measurement rather
than from principle.

**Not fixed, deliberately.** [I1](issues.md#i1) is left un-isolated per [F5](forks.md#f5): the
single shared rule is what makes the question unaskable, and P4 must show the divergence gone.
