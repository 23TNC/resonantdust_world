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

## 2026-08-08 — P2: the corpus and the loader

- Loader: `SpawnEffect` became `Thing { thing, at } | Carried` (TOML untagged —
  a table or the string `"carried"`), `store: Option<String>` beside `remove`,
  location `"slot"` in the whitelist + `location_in_range` (definitionally 0,
  like `self`), spawn-carried-requires-slot-location validation, store's
  carrier-only validation, effect-required includes store. Round-trip test
  (pick_up + drop shapes verbatim) + 4 refusals; 62 lib tests green. The worker's
  spawn arm mechanically rebound to `SpawnEffect::Thing`.
- Content: need `inventory` (0..16, deplete 0 — FREE slots per F1); leveled trait
  `inventory` (`max = [6.0]`); affordance `can_carry = { need = "inventory",
  gt = 0 }`; interactions `pick_up` (adjacent, can_carry, `store = "carrier"`,
  duration 10) and `drop` (location slot, `spawn = "carried"`, inputs
  [pawn, slot]); humans gained the need + trait level 1 + the drop binding;
  pick_up authored on reed, rock, logs, torch, torch_blue, meat, plant_matter
  (F6 — rooted things excluded). Golden re-blessed showing exactly the rows.
- The mint law held with NO new rule (the free-slots inversion):
  `the_inventory_need_mints_all_slots_free` proves a human mints 6.0 on the
  0..16 encoding. Master seeds 231 defs (+5); worker loads 9 interactions (+2);
  wasm/webgl/edge/npc rebuilt; both npc brains re-adopted on the new corpus.

## 2026-08-08 — P3: the worker executes

- The `slot` location (carrier = a validated inventory slot of the acting pawn;
  slot + expected-item inputs, I5 mismatch/empty no-ops), the `store = "carrier"`
  effect (tombstone SET + INV_ADD + free-count SET_NEED in ONE program), the
  `spawn = "carried"` arm (the shared adjacent-empty scan; REFUSE-not-swallow,
  I10), INV verb relays beside SET_NEED, and a pass-local adds count so same-tic
  completions cannot overfill (I2).
- Two integration finds, both fixed:
  - the worker's bundle has NO registry, so `store` could not number the item's
    full def — the index uplink now subscribes `definitions` and the store site
    resolves newest-version variant-0 by kind name (the npc/browser pairing rule);
  - the STALE ORCHESTRATOR could not frame verb 15 (`unframable program —
    UnknownAction(15)`) and silently never assigned the effect event — the I4
    checklist now includes the orchestrator.
- Drills, all live through the REAL pie menu where possible: pick_up on meat
  (walk-then-act; slot 0 = the meat's registry def 0x20000BE0; free 6→5 in the
  same program; the world cell composed to 0; the PawnInventory frame reached the
  browser); the 6th item (a rock) filled freed slot 5 and wrote free=0; the forced
  7th logged the affordance refusal; drop slot 0 put the meat back at (108,66)
  and wrote free=1 with the item-0 frame relayed; a re-drop of the empty slot
  logged the I5 no-op; corpus→0 on the 5-item human left ZERO orphan rows.
