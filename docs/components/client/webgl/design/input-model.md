# Input model — active selection, the context pie menu

The mouse contract (work [`2026-08-06-input-rework`](../../../../work/2026-08-06-input-rework/README.md),
user, 2026-08-06): _"This active selection will move to right click. Left click will become a
context sensitive pie menu. When we left click on an object we will display available
interactions the actively selected object has with the left clicked object… rounded rectangles
around the cursor's x/y… at a fixed distance from the cursor with equal angle spacing… This pie
menu will remain displayed until the user performs an input action… Only one pie menu will be
displayed at any one time."_

## The buttons

| input | meaning |
|---|---|
| **right click** | set the ACTIVE selection (pawn → thing → tile hit order) — details panel, outlines, title suffix: the whole former left-click behavior |
| **left click** | the context PIE MENU: what the active pawn can do with the clicked object; while a menu is open, a rect click executes, anything else dismisses |
| **middle drag** | pan (unchanged) |
| **wheel** | zoom (unchanged; dismisses an open menu) |
| build mode | unchanged: left = place/drag, right = exit build mode (no menu, no selection) |

## The pie menu

- **Content**: the clicked carrier's `interactions = [...]` bindings, filtered by the ACTIVE
  pawn's affordance predicates (`interaction_available` over its fanned trait/condition/needs
  rows — the SAME computation the worker enforces and the npc asks; offering an option the
  worker would refuse is a drift bug) AND the location rule: `"on"` options show only while the
  pawn stands on the clicked tile, `"target"` options always.
- **Composition**: input names are a RESERVED vocabulary — `pawn` = the active selection's
  entity, `destination` = the clicked tile's `position_reference`, `amount` = the carrier
  binding's magnitude (f32 bits). A signature outside the vocabulary is not offered.
- **Presentation**: one rounded rect per option labelled with the interaction's `menu_text`,
  at a FIXED radius from the click point, equal angular spacing starting at 12 o'clock.
  Spacing/fanning refinements are explicitly deferred (user: "We will work on the spacing
  later"). DOM overlay (the ConditionCards pattern — position-fixed sibling of the panels,
  pointer events ON, above the panel z-band), never in-canvas.
- **Lifecycle**: at most ONE menu; opening a new one replaces it; ANY input action dismisses
  it (a rect click executes THEN dismisses; menu clicks never fall through to the canvas).
- **Empty set = NO menu**: no selection, a non-pawn selection, rows not yet fanned, or zero
  available interactions all show nothing — never an empty ring, never a throw.

## Non-goals (recorded, not built)

Pawns as menu TARGETS (social interactions), menu-driven sequencing ("move to and execute"),
ownership (any client may command any pawn — interactions I4), and real passability (walls not
carrying `move_to` is authoring, not a system).
