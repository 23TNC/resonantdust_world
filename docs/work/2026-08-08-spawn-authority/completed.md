# Completed — spawn authority

## 2026-08-08 — P0/P1/P2/P3: the verb, the doors, the authority, the requesters

**P0 ACTIONS.md** — SPAWN_REQUEST palette row (id 17, arity 3, the F1 layout
`[x:16|y:16] [rot:4|type:4|sub:12|kind:12] [v0:4…v7:4]` + the authority law);
CREATE's row now reads WORKER-ONLY. Verified: docs-check green.

**P1 codec** — `SPAWN_REQUEST = 17`, signature `[Imm, Imm, Imm]` (no write set),
`pack_spawn_request` / `unpack_spawn_request`. Verified: unit
`spawn_request_frames_and_round_trips` (frames, empty sets, def+rotation
round-trip) — 16 action tests green.

**P1 edge** — CREATE left CLIENT_VERBS, SPAWN_REQUEST joined (F4). The FULL I1
new-verb sweep ran: event-shard module republished, orchestrator + master +
worker rebuilt and restarted, edge redeployed, `bin/rd build shared` (wasm
typings gained packSpawnRequest), webgl tsc clean. Verified live: a DrillBot WS
client's `[PROMOTE, CREATE, …]` got `queue_err "server-only verb: 3"` at the
door; its SPAWN_REQUEST got queue_ok and reached the worker.

**P2 worker authority arm** — refusals drilled live, each a distinct named log
line ("spawn request refused (spawn-authority F2)"):
- rotation 7 → `rotation out of range (0..3)`
- unregistered def 0x300FFF00 → `def not in the registry (I4)`
- human_male + a third nonzero nibble → `variant nibbles beyond the declared parts (F3)`
- wolf at water (84,100) → `cell impathable (the lake-mint law)` (via /spawn)
- meat at an occupied cell → `cell occupied on the thing layer (F6)`
Nothing minted on any refusal.

**P2 mint_parts** — unit `mint_parts_matches_the_chat_words` (a human + nibbles
(5,12) yields the exact PART words the chat used to pack; a single-part def
takes nibble 0 into its own variant). Verified live: wolf request rot=2 minted
→ pawn row data=128 (rotation bits = 2, serial 0) — the facing seeds from the
request (I9).

**P2 TYPE router (F6)** — meat requests at empty cells (95,42) and (93,40)
queued cold SETs ("spawn request queued a cold SET", kind_reference 0x00e0,
layer 2) and RENDER in the client; re-hitting (95,42) after the SET landed
refused `cell occupied` — the mint→occupied round trip. Tile-replace rides the
same lane (the build lane, SET on the tile layer) — not separately drilled.

**P3 webgl /spawn** — part-packing deleted; the command packs a request via the
wasm `packSpawnRequest` and echoes "Spawn requested: … the server decides".
Verified live in the browser: `/spawn human_male 100 48 body 5 head 12` →
worker "spawn request minted … def=0x300200b0 parts=2" → pawn payload rows
decode part 0 = 0x300200B5 (body 5), part 1 = 0x300200BC (head 12) — the
variants land in SQL. The human renders at (100,48).

**P3 npc requesters** — wolves + bunnies replaced `[PROMOTE, CREATE, …]` with
`pack_spawn_request` (pathable picker kept as POLITENESS — I2); both gained the
refusal-retry posture (wolves: an unanswered request past the window clears
`created` and re-rolls; bunnies: `created` hands back to `minds.len()` on
stall). Verified live on the wiped world: wolf requested → minted → adopted →
trips; bunny "SPAWN_REQUEST queued … requested=3" → "bunny adopted". The
forced-refusal re-roll is exercised in the P4 cold-boot drill (worker-down
window), not by a water pick — the politeness picker makes a natural water
request unreachable.

## 2026-08-08 — P4: the unified drill + cold boot

**The unified drill** ran across one session (all in the log record above +
this): the DrillBot WS client's CREATE refused at the door and its requests
minted/refused correctly; `/spawn human_male 100 48 body 5 head 12` landed
both variants in SQL; `/spawn wolf 84 100` (a lake cell found by decoding the
tile shard) refused `cell impathable`; the warren filled by requests
("bunny SPAWN_REQUEST queued … bunny adopted").

**Cold boot** (full stack: master, orchestrator, worker, edge, npc, bunnies):
first bounce EXPOSED a race — the 3 s adopt-first windows lost to the cold
edge's late zone snapshot, and BOTH brains minted a duplicate (a 2nd wolf, a
4th bunny). Widened to 10 s (wolves + bunnies); the second bounce re-adopted
the whole cast with ZERO new mints (pawn count stable at 8), and a post-bounce
rotation-7 request drilled the full path end-to-end ("rotation out of range"
in the worker log). The duplicates were left alive — more traffic for P5.

## 2026-08-08 — P5: the teleport hunt — REPRODUCED, two causes fixed, one named

**Probes** (I7's AUTH/RENDER split): client-side `__teleportProbe` in
MoverLayer — AUTH events (consecutive authoritative rows striding beyond
0.15 tiles/tic × the tic gap + slack) and RENDER events (the chase snapping
past 3 tiles), ring-buffered + console-mirrored; server-side a polling watcher
over `entity_state_log` (scratchpad `auth_probe.py`). First lesson: the flat
2.5-tile threshold flagged ordinary walking — rows only arrive every ~32 tics
(the re-anchor cadence) — so the client probe scales by the row tic gap now.

**Reproduction**: a long cross-zone trip + a mid-trip interrupt teleported the
human INSIDE FIVE MINUTES of arming the probes — AUTH strides of 7.69, 9.19,
8.81 tiles per ~32 tics with paired RENDER snaps. Three causes, correlated to
their writes ([I12](issues.md#i12)/[I13](issues.md#i13)/[I14](issues.md#i14)):
zone re-subscribes replaying history into freshly-recreated movers (client —
FIXED, the rejection lines fire live), cross-zone orders executing once per
work-group producing twin leapfrogging chains (server — FIXED by the worker's
executed-interactions dedup; the orchestrator's split assignment named as the
structural successor), and orders executing ~1000 tics late when their zones
sit outside every active work-group (server — NAMED, structural). Post-fix
drills: the same cross-zone trip + interrupt executes each order exactly once
and the replay rejections land on every crossing.

**Found live during the sweep** — the worker's SPAWN_REQUEST match arm compiled
as a BINDING pattern (the const wasn't imported), swallowing every verb in the
spawn pre-pass and logging every 3-operand instruction (MOVE_STEP hops,
QUEUE_STATE fans) as a bogus "position out of world" refusal. Fixed by
importing the const; logged as the I11 lesson in issues.md.
