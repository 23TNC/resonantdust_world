# Input rework — active selection, the pie menu, movement as an interaction

_Opened 2026-08-06 (user): "we are going to implement the concept of an active selection. This
will be what our current selection handles populating our details panel and causing outlines.
This active selection will move to right click. Left click will become a context sensitive pie
menu. When we left click on an object we will display available interactions the actively
selected object has with the left clicked object. We will display these options as rounded
rectangles around the cursor's x/y… display each rounded rect at a fixed distance from the
cursor with equal angle spacing between each option. This pie menu will remain displayed until
the user performs an input action… Only one pie menu will be displayed at any one time… We will
add a variable to our interaction toml, menu_text 'Move To', and our rounded rectangles will be
populated with menu_text. Finally we will replace our existing move events with our new
interaction based movement system." Decisions verbatim in [`forks.md`](forks.md)._

The declared successor of [2026-08-06-stat-model](../2026-08-06-stat-model/README.md), which
pre-authored its ground: `walks` → `ground_speed` → `can_move_ground` already exist and derive;
this stream gives them their consumer and deletes the two-speed window (stat-model F12/I10).

## The model

**The input contract** ([F1](forks.md#f1), user): RIGHT click = the ACTIVE selection — panel,
outlines, everything the current left-click selection does. LEFT click = the context pie menu:
the interactions the clicked object OFFERS that the active pawn MAY use (the same
`interaction_available` + carrier bindings the worker and npc already share), one `menu_text`
rounded rect per option at a fixed radius with equal angular spacing, anchored at the cursor.
One menu at a time; ANY input action dismisses it; choosing a rect composes and queues the
interaction. No selection or no options → no menu ([F7](forks.md#f7)).

**Movement becomes the `move_to` interaction** ([F2](forks.md#f2), user): a corpus
`[[interaction]]` gated by `can_move_ground`, carried by the GROUND tiles (walls simply don't
carry it — the first soft passability, by authoring, [F9](forks.md#f9)), with the new `move`
effect (`move = { target = "@pawn", to = "@destination" }`, [F6](forks.md#f6)) and the new
location rule `"target"` (the CLICKED tile is the carrier; drink's `"on"` stays,
[F4](forks.md#f4)). The worker's arm validates and queues the `PROMOTE MOVE_TO` chain seed —
`MOVE_TO` leaves `CLIENT_VERBS` and becomes WORKER-ONLY ([F3](forks.md#f3)); the `MOVE_STEP`
chain, trip-serial supersession, and the fanned-intent speculation channel all survive
untouched.

**Speed is the DERIVED stat everywhere** ([F8](forks.md#f8)): the worker's chain spacing, the
npc's deadlines, and the client's speculation all read `ground_speed` from the pawn's rows +
corpus through the ONE `stat_eval`. The `speed` field, the speeds table, `thingSpeed`, and the
stat-model I10 guard DIE here, as planned.

**The menu is generic** ([F5](forks.md#f5)): input names are a RESERVED vocabulary the UI knows
how to bind — `pawn` = the active selection, `destination` = the clicked position, `amount` =
the carrier binding's magnitude. Drink re-authors to `inputs = ["pawn", "amount"]` with its
need BAKED (`need = "thirst"`), because a generic menu cannot guess a need input.

## Design stance

- ONE availability computation, still: the menu asks the same corpus questions the worker
  enforces and the npc asks — a menu option that would be refused is a drift bug, not UX.
- The pie menu is a DOM overlay (the ConditionCards pattern), screen-anchored; spacing beyond
  fixed-radius-equal-angles is explicitly LATER (user).
- Replace means DELETE: the raw `MOVE_TO` front door, `move_to_program`, `Command::Move`,
  `moveEntity`, the `speed` field, and the legacy `wildlife` brain (the superseded regression
  fixture that drives client-minted wolves through the dying door, [F10](forks.md#f10)) all go.
- "Move to and execute" (queued sequencing) stays the recorded successor — the menu fires
  interactions; it does not chain them.

## Exit

In the browser: right-click selects a wolf (panel + outline), left-click on water pops rounded
rects — **Drink** (only while standing on it) and **Move To** — choosing Move To walks the wolf
there with the speculation glide intact at 12 tics/tile derived from `walks`; the npc's wander
and drink arcs run unchanged on the same interaction door; a cold boot holds. Captures + logs
in `completed.md`; **the user's eyes close the stream**.
