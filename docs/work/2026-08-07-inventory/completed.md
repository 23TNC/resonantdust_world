# Completed — inventory

## 2026-08-07 — P0: the paper

- VARIABLES.md: the inventory need documented beside corpus (FREE slots, 0..16,
  deplete 0 — the mint-at-effective-max law is correct as-is), the four-families
  paragraph (need = free count, trait = capacity, rows = items, PawnInventory fan,
  INV verbs + SET_NEED one-program law), and full `pick_up`/`drop` interaction
  schema examples introducing `store = "carrier"`, `location = "slot"`, and
  `spawn = "carried"` (refuse-not-swallow noted). Verified: docs-check green.
- TABLES.md: the `inventory` sub-table beside `needs` — uid `entity:32|slot:8`,
  slaved zone key, `item` = definition_reference, `state` RESERVED for the
  item-as-entity successor, worker-only mutation, no log twin. Verified:
  docs-check green.

## 2026-08-08 — P1: the shard and the wire

- Pawn module: the `Inventory` table (first-free-slot `inv_add`, silent-absent
  `inv_remove`, the structural 16-slot bound erring LOUD), the state hook drags
  inventory zone keys beside needs, `remove` deletes a dead pawn's rows (items are
  NOT spilled — the death spawn already yields; spill is the item-as-entity
  successor's call). Deployed targeted (`deploy module pawn` — the full redeploy
  would have wiped every shard); bindings regenerated for edge + st-bindings;
  master/worker/npc rebuilt (the stale-binary guard caught each). Verified live:
  CLI inv_add seeded slot 0, remove cleared it, a second remove no-opped.
- Codec: `INV_ADD = 15` / `INV_REMOVE = 16`, both `[Write, Imm]` — the obj rides
  the write set so inventory mutations serialise with the pawn's claim. WORKER-ONLY
  by CLIENT_VERBS omission (comment records it). Event-shard module redeployed
  (I4); a CLI-queued INV_ADD program framed and landed in event_log.
- The wire: `Inventory` frame (edge protocol + fan on insert/update/delete —
  delete relays `item = 0`, the slot-emptied signal, because the pawn LIVES on and
  no StateGone drops the join), zone-snapshot replay beside needs, worker
  subscription `SELECT * FROM inventory`, `Event::PawnInventory` through
  client/core (engine + web), the wasm emit, and `WasmClient.onPawnInventory`.
  Verified in the browser: insert → `{slot:0, item:805372112}`, delete →
  `{item:0}`, both via a live CLI drill against the wolf.
