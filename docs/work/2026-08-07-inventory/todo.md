# Plan — inventory

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions
in [`forks.md`](forks.md) (F#), the anticipated-issue inventory in
[`issues.md`](issues.md) (I#)._

## P0 — the paper

- [ ] VARIABLES.md: the inventory need (0..16 encoding, count-as-value, MINT-EMPTY
      rule — F1/I2), the leveled trait, the `inventory` sub-table shape + PawnInventory
      frame (F2), INV_ADD/INV_REMOVE (F3), `below_max`/`slot`/`spawn = "carried"`/
      `store` vocabulary (F4/F5). Acceptance: docs-check green.
- [ ] TABLES.md: the pawn-shard `inventory` table row (uid, entity, macro, slot,
      item, state — F2, state RESERVED). Acceptance: docs-check green.

## P1 — the shard and the wire

- [ ] Pawn module: the `inventory` table + `remove` deletes its rows; redeploy
      sequenced (I1); bindings regenerated (edge + st-bindings). Acceptance: live SQL
      shows the empty table; a CLI remove on a seeded row clears it.
- [ ] Codec: INV_ADD/INV_REMOVE verbs + arities; the edge verb allowlist grows;
      event-shard module redeployed (I4). Acceptance: a drill INV_ADD from the worker
      lands a row live (not async-rejected).
- [ ] Worker subscription: `SELECT * FROM inventory` (build-gates: verify LIVE);
      edge fans PawnInventory frames per row mutation (the PawnNeed pattern).
      Acceptance: a drill row reaches a browser client's console.

## P2 — the corpus and the loader

- [ ] Loader: check form `below_max` (F4 — absent row never passes; value < effective
      max via need_bounds); location rule `"slot"`; effect `spawn = "carried"` +
      `store = "carrier"`; refusals for misuse. Acceptance: round-trip + refusal
      tests green.
- [ ] needs: mint rule (`mint = "empty"` — I2) parsed + honored by mint_sidecars.
      Acceptance: unit test — thirst mints full, inventory mints 0.
- [ ] Content: need `inventory` (0..16, deplete 0, mint empty); leveled trait
      `inventory` (max = [6]); humans get {inventory, 1}; affordance can_carry.
      Acceptance: golden diff = the authored rows.
- [ ] Content: `pick_up` (adjacent, can_carry, store, duration 10) on meat/
      plant_matter/logs/rock/reed/torch/torch_blue (F6); `drop` (location slot,
      spawn carried) on human kinds. Acceptance: golden re-blessed; six consumers
      rebuilt (I8).

## P3 — the worker executes

- [ ] Worker: `store = "carrier"` — tombstone SET + INV_ADD(first free slot) +
      SET_NEED count, ONE program (F3/I3); completion re-validates carrier + capacity
      (I7). Acceptance: drill — walk-then-pick-up a log; row + count 1; the log gone
      from the world.
- [ ] Worker: full-capacity refusal — can_carry gates at queue AND completion; the
      6th item fills, the 7th refuses. Acceptance: drill — a full human's pick_up
      logs the refusal; count stays 6.
- [ ] Worker: `drop` — slot carrier resolve (inputs slot + expected item, mismatch
      no-op — I5), spawn carried at the adjacent scan, REFUSE when no cell (I10),
      INV_REMOVE + SET_NEED. Acceptance: drill — drop slot 0; the thing reappears
      beside the pawn; count decrements.
- [ ] Worker: death/remove clears inventory rows (the remove reducer already
      deletes them — verify the composed path). Acceptance: kill a carrying pawn;
      no orphan inventory rows in SQL.

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
