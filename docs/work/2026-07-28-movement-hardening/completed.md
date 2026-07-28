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

*Commit note*: a concurrently-active session (art-128-tiles) ran `git add -A` mid-P1, so this
phase's CODE landed inside its commits `ca98cf0` + `6684ab5` (art-titled messages). Nothing
lost — verified file-by-file — but attribution interleaves; this session stages selectively
from here on while both are live.
