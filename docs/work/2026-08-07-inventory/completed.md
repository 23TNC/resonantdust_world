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
