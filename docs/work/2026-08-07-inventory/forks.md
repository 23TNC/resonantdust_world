# Forks — inventory

## F1 — the need is the count; the leveled trait is the capacity {#f1}

Options: (a) a bespoke capacity field on kinds; (b) a stat; (c) the user's shape — a
need whose value counts and whose max caps. **(c)**: need `inventory`, encoding
domain 0..16 (headroom for future tiers), `deplete = 0`, no bands; value = held
count, worker-written. The leveled `inventory` trait (`needs = [{ need =
"inventory", max = [6] }]`) provides + caps it — humans `{inventory, 1}` = 6. The
food-chain laws carry over unchanged: HIGHEST-wins caps, effective max via
need_bounds, the need-write TRIGGER now fires on inventory mutations (future "on
full" behaviors are free). Rejected (a)/(b): a second capacity vocabulary beside
the one the corpus trait already proved.

## F2 — items are sub-table rows; `state` is the reserved future {#f2}

`inventory(uid, entity_reference, macro_position_reference, slot u8, item u32,
state u32)` in the PAWN shard, fanned as `PawnInventory` (the PawnNeed pattern:
per-row frames, entity-keyed). `item` = definition_reference (the user's u32).
`state` = 0 today, RESERVED: the documented successor mints items as ENTITIES —
a held item's world row is suppressed, `state` carries its entity id, drop
restores it, and "where is it" becomes a real location. That successor is NOT
built; the reserved word + this paragraph preserve the intent (the user's closing
worry, answered without scope). Rejected: packing items into payload rows — the
payload is trait/condition/part vocabulary; a list with slots wants a table.

## F3 — INV_ADD / INV_REMOVE, and the count writes beside them {#f3}

Two worker-only event verbs: `INV_ADD(entity, item)` appends at the first free
slot; `INV_REMOVE(entity, slot)` clears one. EVERY mutation program also carries
`SET_NEED inventory = <new count>` — one author (the worker's effect composer),
one atomic program, so rows and count cannot diverge (I3). The edge verb
allowlist gains both; the event-shard module redeploys (I4 — the async-invisible
reject law). Rejected: deriving the need lazily from row count — it would
special-case one need inside the ONE needs eval and break the trigger law's
"a write is the trigger".

## F4 — pick_up: adjacent walk-then-act, `below_max`, `store = "carrier"` {#f4}

`pick_up` on portable things, location `adjacent` (cheb ≤ 1, the lumberjack
compose), duration 10. Affordance `can_carry` = the NEW check form
`{ need = "inventory", below_max = true }`: passes iff the row EXISTS (absent
never passes — the standing law gives "has inventory need" free) AND lazy value <
the EFFECTIVE max (need_bounds, so capacity tiers keep working). Effect `store =
"carrier"`: the destroy tombstone SET (the thing leaves the world) + INV_ADD of
the carrier's kind def + the count SET_NEED, one program. Completion re-validates
(lumberjack law) — two pawns racing one log: the loser logs a NO-OP (I7).

## F5 — drop: ONE interaction, location `"slot"`, `spawn = "carried"` {#f5}

The user's open question — the generic shape. Drop is authored ONCE, on the
inventory-bearing PAWN kinds (the death `location = "self"` precedent): a NEW
location rule `"slot"` makes the carrier an inventory slot of the ACTING pawn;
the slot index + EXPECTED item def ride the event inputs, and the completion
re-validates the slot still holds that item (I5 — slots mutate under queued
intents). Effect `spawn = "carried"`: the food-chain adjacent scan (first EMPTY
pathable cell) places the SLOT's kind, then INV_REMOVE + the count SET_NEED. One
drop covers every item because the effect names the CARRIED def, never a fixed
thing. No empty pathable neighbor → REFUSE and keep the item (I10). Rejected:
per-thing drop interactions (N copies of the same verb); an item-side location
"inventory" lane (real, but the successor — in-hand verbs like eat-from-hand
join F5's slot menu later without reshaping it).

## F6 — which things get pick_up {#f6}

Portable = meat, plant_matter, logs, rock, reed, torch, torch_blue. Rooted stays
rooted: tree/shrub/cactus (fell first — their verb is cut_down), flora (forage
is its verb). Pawn kinds never. The user said "all… most things" — this is the
line drawn; it is a TOML edit to move it. Torches picking up = a light leaves
the world via the tombstone lane — a nice stress of the render path.

## F7 — the panels: name+tile details, a gated button row, a gated grid {#f7}

The details panel DROPS its text block (the user's ask) and shows the def's TOML
`name` + the object's live tile — cards/strip/wash stay. A TOP BUTTON ROW opens
sibling panels: [Inventory] renders iff the SELECTED kind's corpus lists the
inventory need (a bundle lookup, no wire read). The inventory panel shows the
ACTIVE pawn's slots (effective max, so a level-2 trait widens the grid) as
squares — placeholder tint + name tooltip per item, fed by the PawnInventory fan
— and does not render at all for inventory-less objects. A slot click opens the
pie menu ON the slot (input-rework menu_text rects) listing the pawn's
slot-located interactions through the ONE wasm availability filter, which gains
the slot context. Display names: the TOML `name` as-is (human_male, meat) — a
`display` field is a recorded nicety, not this stream (I9).
