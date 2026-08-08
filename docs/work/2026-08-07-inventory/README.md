# Inventory — the trait, the need, the panel, pick up and drop

**What** (user, 2026-08-07): a trait INVENTORY providing the INVENTORY need; the need
determines how many items a pawn may hold. An inventory is populated with u32 items.
Humans get a max inventory of 6. The details panel gains a BUTTON ROW along the top
opening a new INVENTORY panel showing the ACTIVE pawn's inventory. The details
panel's existing text is REMOVED; it shows the object's NAME (from the TOML — human,
wolf, meat…) and its TILE. No inventory need on the selected object → no button; no
inventory on the active object → no panel. A PICK UP interaction — affordance: HAS
the inventory need AND NOT at max — adds the item to the pawn's inventory and raises
the inventory need by 1. The panel draws items as SQUARES in a grid; clicking a
square prompts for interactions on the item in that slot. Pick up goes on all
current things ("most things"). A DROP interaction too — the user asked for a
generic shape (and flagged that u32 defs may not carry enough object STATE — "so it
understands where it is"). Plan review flipped the need's sense: it counts FREE
slots ("at 0 we have 0 free slots"), so pick up DECREMENTS it; and confirmed
TOML names in the panel — the point is identification (today `thing #26335,
texture (geo)` hides that a green square is a shrub).

## The stance

- **The need counts FREE SLOTS; the trait is the capacity** ([F1](forks.md#f1) —
  the user's plan-review correction: "the inventory need becomes free slots…
  at 0 we have 0 free slots"): need `inventory` on a 0..16 encoding domain,
  `deplete = 0`, no bands — VALUE = free slots, worker-written (the food-chain
  need-write TRIGGER fires on inventory changes for free). ONE leveled
  `inventory` trait (`max = [6]`, the corpus pattern) provides the need and caps
  it — humans level 1 = capacity 6; future levels (backpacks) append slots. The
  standing mint-at-effective-max law is CORRECT as-is (all slots free at birth);
  free vs filled must never conflate ([I2](issues.md#i2)).
- **Items are rows in a pawn-shard sub-table** ([F2](forks.md#f2)):
  `inventory(entity, slot, item, state)` — `item` = the u32 definition_reference,
  `state` a RESERVED word (0 today). The user's where-is-it worry is answered by the
  documented successor, NOT built now: items become ENTITIES (minted ids whose world
  row is suppressed while held; drop restores it) and `state` carries the id. Fanned
  to clients as `PawnInventory` frames (the PawnNeed pattern). The pawn module
  changes → the redeploy WIPES live pawns ([I1](issues.md#i1)); `remove` learns to
  delete inventory rows.
- **Two worker-only verbs move items** ([F3](forks.md#f3)): `INV_ADD(entity, item)`
  (first free slot) and `INV_REMOVE(entity, slot)`; every mutation program ALSO
  writes `SET_NEED inventory = new count` — one author, one atomic program, count
  and rows never diverge ([I3](issues.md#i3)). New codec verbs = event-shard module
  redeploy + edge allowlist ([I4](issues.md#i4)).
- **pick_up is walk-then-act on the thing** ([F4](forks.md#f4)): location
  `adjacent`, affordance `can_carry` = the EXISTING `{ need = "inventory",
  gt = 0 }` check ("has the need" falls out — a check on an absent row never
  passes; "not max" = a free slot remains; no new check machinery). Effect
  `store = "carrier"`: the destroy-lane tombstone SET + INV_ADD + `SET_NEED
  free − 1`, one program. Authored on every portable thing ([F6](forks.md#f6)).
- **drop is ONE generic interaction on the PAWN** ([F5](forks.md#f5) — the user's
  open question): carried by inventory-bearing pawn kinds (the death `self`
  precedent), NEW location rule `"slot"` — the carrier is an inventory slot of the
  acting pawn, the slot index + expected item ride the event inputs (a completion
  re-validates the slot still holds that item — [I5](issues.md#i5)). Effect
  `spawn = "carried"`: the food-chain adjacent scan places the SLOT's kind beside
  the pawn — because the effect names the carried item, not a fixed thing, one drop
  covers every item; then `INV_REMOVE` + `SET_NEED free + 1`. No empty pathable
  cell → REFUSE, the item stays held (never the forage all-full-swallows rule —
  [I10](issues.md#i10)).
- **The panels** ([F7](forks.md#f7)): the details panel drops its text block and
  shows NAME (the def's TOML name) + TILE; a top button row gains [Inventory],
  shown iff the selected kind's corpus carries the inventory need. The inventory
  panel renders effective-max slots as a grid of squares (placeholder tints; the
  PawnInventory fan fills them) for the ACTIVE pawn only, hidden for objects
  without the need. A slot click opens the pie menu on that slot — the pawn's
  slot-located interactions (drop today; future in-hand verbs join the same lane)
  through the ONE wasm availability filter.

Authoritative docs touched: VARIABLES.md (the need/trait, the inventory table
shape, the verbs, the check/effect/location vocabulary), TABLES.md (the sub-table).
