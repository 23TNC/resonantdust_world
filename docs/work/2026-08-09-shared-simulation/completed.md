# Completed — shared-simulation

_The verification log: what landed and **how it was checked**. Append-only; authoritative for what
is done. Items live in [`todo.md`](todo.md) with their boxes ticked._

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
