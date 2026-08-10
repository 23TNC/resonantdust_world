# Completed — shared-simulation

_The verification log: what landed and **how it was checked**. Append-only; authoritative for what
is done. Items live in [`todo.md`](todo.md) with their boxes ticked._

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
