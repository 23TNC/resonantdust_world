# Plan — inventory

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions
in [`forks.md`](forks.md) (F#), the anticipated-issue inventory in
[`issues.md`](issues.md) (I#)._

## P0 — the paper

- [x] VARIABLES.md: the inventory need (0..16 encoding, FREE-SLOTS-as-value —
      F1/I2; standing mint law correct as-is), the leveled trait, the `inventory`
      sub-table shape + PawnInventory frame (F2), INV_ADD/INV_REMOVE (F3),
      `slot`/`spawn = "carried"`/`store` vocabulary (F4/F5). Acceptance: docs-check
      green. → the need comment, the four-families paragraph, and full pick_up/drop
      schema examples; green.
- [x] TABLES.md: the pawn-shard `inventory` table row (uid, entity, macro, slot,
      item, state — F2, state RESERVED). Acceptance: docs-check green. → the
      sub-table section beside `needs` (uid = entity:32|slot:8, slaved zone key,
      the never-conflate law); green.

## P1 — the shard and the wire

- [x] Pawn module: the `inventory` table + `remove` deletes its rows; redeploy
      sequenced (I1); bindings regenerated (edge + st-bindings). Acceptance: live SQL
      shows the empty table; a CLI remove on a seeded row clears it. → drilled both:
      inv_add seeded slot 0 (uid = entity<<8), remove wiped it; cast re-minted.
- [x] Codec: INV_ADD/INV_REMOVE verbs + arities; the edge verb allowlist grows;
      event-shard module redeployed (I4). Acceptance: a drill INV_ADD from the worker
      lands a row live (not async-rejected). → verbs 15/16 ([Write, Imm]); WORKER-ONLY
      by allowlist omission (commented); a queued INV_ADD program framed + accepted
      into event_log post-redeploy.
- [x] Worker subscription: `SELECT * FROM inventory` (build-gates: verify LIVE);
      edge fans PawnInventory frames per row mutation (the PawnNeed pattern).
      Acceptance: a drill row reaches a browser client's console. → both directions
      in the browser: insert → {slot:0, item:805372112}, delete → {item:0} (the
      slot-emptied relay); zone-snapshot replay wired beside needs.

## P2 — the corpus and the loader

- [x] Loader: location rule `"slot"`; effect `spawn = "carried"` + `store =
      "carrier"`; refusals for misuse (store on a tile carrier, carried without a
      slot location…). Acceptance: round-trip + refusal tests green. → SpawnEffect
      became Thing|Carried (untagged TOML), store beside remove, slot in the
      whitelist + range check; 4 refusals tested; 62 lib tests green.
- [x] Content: need `inventory` (0..16, deplete 0 — FREE slots, F1); leveled trait
      `inventory` (max = [6]); humans get {inventory, 1}; affordance can_carry =
      `{need inventory, gt 0}`. Acceptance: golden diff = the authored rows; mint
      unit test — a human mints 6 free, 0 rows (I2). → golden shows exactly the
      rows; `the_inventory_need_mints_all_slots_free` proves 6.0 on 0..16.
- [x] Content: `pick_up` (adjacent, can_carry, store, duration 10) on meat/
      plant_matter/logs/rock/reed/torch/torch_blue (F6); `drop` (location slot,
      spawn carried) on human kinds. Acceptance: golden re-blessed; six consumers
      rebuilt (I8). → blessed; master seeds 231 (+5), worker 9 interactions (+2);
      both npc brains healthy on the new corpus.

## P3 — the worker executes

- [x] Worker: `store = "carrier"` — tombstone SET + INV_ADD(first free slot) +
      `SET_NEED free − 1`, ONE program (F3/I3); completion re-validates carrier +
      free slot (I7). Acceptance: drill — walk-then-pick-up a log; 1 row, need 5;
      the log gone from the world. → pie-menu drill on MEAT: walk-then-act, slot 0
      = the meat's registry def, free 6→5 same-program, the cell composed to 0.
      FOUND: the worker's bundle has no registry — item defs come from the index's
      `definitions` table; and the STALE ORCHESTRATOR rejected verb 15 (I4 bit —
      unframable program) until rebuilt.
- [x] Worker: full-capacity refusal — can_carry (`gt 0`) gates at queue AND
      completion; the 6th item fills, the 7th refuses. Acceptance: drill — a full
      human's pick_up logs the refusal; 6 rows, need 0. → the 6th (a rock) filled
      the freed slot 5 and wrote free=0; the forced 7th logged `an affordance
      predicate fails` — rows stayed 6.
- [x] Worker: `drop` — slot carrier resolve (inputs slot + expected item, mismatch
      no-op — I5), spawn carried at the adjacent scan, REFUSE when no cell (I10),
      INV_REMOVE + `SET_NEED free + 1`. Acceptance: drill — drop slot 0; the thing
      reappears beside the pawn; rows 0, need 6. → drop slot 0: meat REAPPEARED at
      (108,66) (the first-empty scan), row deleted, free 0→1, the item-0 frame
      relayed; a re-drop of the empty slot logged the I5 no-op.
- [x] Worker: death/remove clears inventory rows (the remove reducer already
      deletes them — verify the composed path). Acceptance: kill a carrying pawn;
      no orphan inventory rows in SQL. → corpus→0 on the 5-item human: trigger →
      death → removed; the inventory table shows ZERO rows for it.

## P4 — the client

- [ ] Details panel: REMOVE the text block; render NAME (the def's TOML name) +
      TILE, live (F7/I9). Acceptance: capture — a selected human shows
      "human_male (114, 74)"-class content, no legacy text.
- [ ] Details panel: the top BUTTON ROW (I11) with [Inventory], rendered iff the
      selected kind's corpus lists the inventory need. Acceptance: capture — button
      on a human, ABSENT on a wolf and a tree.
- [ ] Inventory panel: effective-max slots as a square grid, PawnInventory-fed
      (placeholder tint + name tooltip per item), ACTIVE pawn only, hidden for
      inventory-less objects (F7). Acceptance: capture — 6 slots, 1 filled after a
      pick_up; panel gone when a wolf is selected.
- [ ] Slot click → the pie menu on that slot: the pawn's slot-located interactions
      via the wasm availability filter with the slot context (F5/I6); pick_up grays
      when full in the THING pie menu. Acceptance: capture — drop offered on a
      filled slot; a full pawn's log menu grays pick_up.

## P5 — the verdict

- [ ] The chain drill: pick up a log → the panel fills → walk away → drop → the
      log reappears beside the pawn (world + reload-persist) → pick it back up.
      Acceptance: captures + worker logs of every link.
- [ ] Docs + memory truth pass + a stack bounce with arcs green; **the user's eyes
      close the stream**. Acceptance: docs-check green; captures + logs in
      completed.md.
