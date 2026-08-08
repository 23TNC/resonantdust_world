# Plan — spawn authority

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions
in [`forks.md`](forks.md) (F#), the anticipated-issue inventory in
[`issues.md`](issues.md) (I#)._

## P0 — the paper

- [ ] ACTIONS.md: the `SPAWN_REQUEST def position variants` verb (request → the
      worker validates → queues the worker-only CREATE — F1/F2/F3) + the spawn
      AUTHORITY law; VARIABLES.md loses CREATE's client-open note. Acceptance:
      docs-check green.

## P1 — the verb and the doors

- [ ] Codec: `SPAWN_REQUEST` (3 × Imm — [x:16|y:16] [rot:4|type:4|sub:12|kind:12]
      [v0:4…v7:4], the F1 layout; no write set); CREATE stays as-is. Acceptance:
      unit — the program frames; write/read sets empty; the def reconstruction
      round-trips (word ↔ def + rotation).
- [ ] Edge: SPAWN_REQUEST joins CLIENT_VERBS, CREATE LEAVES it (F4); the FULL
      new-verb sweep (I1: event-shard module, orchestrator, edge, worker, wasm,
      webgl). Acceptance: a queued client CREATE is refused at the door; a
      SPAWN_REQUEST lands in event_log.

## P2 — the worker's authority

- [ ] Worker: the SPAWN_REQUEST arm — reconstruct def + rotation from word 2;
      validate the def against the definitions mirror + corpus (I4), the position
      in-world + PATHABLE (F2, refuse loudly), rotation ≤3, and the variant
      nibbles against the CORPUS PART COUNT (F3 — extra nonzero nibbles refuse).
      Acceptance: bad def / water position / rotation 7 / a third nibble on a
      human all log distinct refusals; nothing mints.
- [ ] Worker: `mint_parts` — PART entries composed server-side per the kind's
      declared parts (nibble i → part i's variant; a single-part kind takes
      nibble 0 into its own def variant), then queue `PROMOTE CREATE def pos
      count payload…` worker-side (one per request — I5), the CREATE seeding the
      requested FACING. Acceptance: unit — a human + nibbles (5, 12) yields the
      exact PART words the chat used to pack; the wolf's nibble 0 lands in its
      def variant; the minted row carries the rotation.

## P3 — the clients become requesters

- [ ] webgl `/spawn`: DELETE the part-packing; send `SPAWN_REQUEST def pos
      (body<<4|head)`; echo says "requested" (I3). Acceptance: /spawn human_male
      body 5 head 12 mints a two-part human with the variants (SQL payload).
- [ ] npc: wolves + bunnies mint via SPAWN_REQUEST (keep the pathable pick as
      politeness — I2) and RETRY on a refusal instead of latching created
      (I2/I6). Acceptance: both brains mint on a clean world; a forced water
      request refuses and the brain re-rolls.

## P4 — the drills

- [ ] The unified drill: /spawn (variants land), the npc warren fills, a client
      CREATE refused at the edge, a water /spawn refused at the worker — one
      session, logs + SQL for each. Acceptance: captures + log lines in
      completed.md.
- [ ] Cold boot: bounce the stack; brains re-adopt (no duplicate mints), /spawn
      still works, the request path survives the event-shard's clean queue.
      Acceptance: arcs green post-bounce.

## P5 — the teleport hunt, and the verdict

- [ ] The probes (F5/I7): AUTH — consecutive entity_state_log positions per pawn
      jumping beyond one hop stride; RENDER — belief-vs-auth divergence beyond
      the chase envelope; both timestamped and tagged. Acceptance: the probes
      run against live traffic and log indexable events.
- [ ] Reproduce + correlate: npc wander + long trips + mid-trip interrupts +
      zone crossings; every jump correlated to its writes (row dumps).
      Conditions land in issues.md; cheap causes fixed, structural ones named.
      Acceptance: issues.md holds identified conditions + evidence, or the
      honest no-repro record with what was tried.
- [ ] Docs + memory truth pass + a stack bounce with arcs green; **the user's
      eyes close the stream**. Acceptance: docs-check green; captures + logs in
      completed.md.
