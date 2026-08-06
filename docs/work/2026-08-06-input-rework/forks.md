# Forks — input-rework

## F1 — the input contract: right = ACTIVE selection, left = the context pie menu {#f1}

_2026-08-06, **user**: "This active selection will move to right click. Left click will become
a context sensitive pie menu. When we left click on an object we will display available
interactions the actively selected object has with the left clicked object… rounded rectangles
around the cursor's x/y… a fixed distance from the cursor with equal angle spacing… This pie
menu will remain displayed until the user performs an input action. This action could be to
select one of the rounded rects or another action entirely which will cause the pie menu to
become hidden. Only one pie menu will be displayed at any one time."_

**Chosen (user)**: verbatim. The current left-click selection behavior (panel, outlines) moves
WHOLE to right click; left click asks "what can my active pawn do with THAT". Spacing beyond
fixed-radius-equal-angles is explicitly deferred by the user ("We will work on the spacing
later").

## F2 — movement is the `move_to` interaction; the ground work landed in stat-model {#f2}

_2026-08-06, **user**: "We already have interactions, so… we are going to make a 'Ground Move'
interaction, and apply that interaction to our various tiles… We will add a variable to our
interaction toml, menu_text 'Move To'… Finally we will replace our existing move events with
our new interaction based movement system."_

**Chosen (user)**: `[[interaction]] move_to` (`menu_text = "Move To"`), gated by the
already-authored `can_move_ground`, carried by ground tiles, replacing the raw MOVE_TO front
door. The trait/stat/affordance half ("Walks"/"Ground Move") was delivered by stat-model F5/F12
— this stream is the consumer half.

## F3 — MOVE_TO becomes WORKER-ONLY; the chain and speculation survive untouched {#f3}

_2026-08-06._ "Replace our existing move events" = replace the FRONT DOOR, not the machinery:
`MOVE_TO` leaves the edge's `CLIENT_VERBS` (joining `MOVE_STEP` as server-only) and the
worker's `move_to` interaction arm becomes its only composer — it queues the same
`PROMOTE MOVE_TO pawn dest` seed the client used to send. Everything downstream is untouched:
`apply` stamps the trip serial from the (new) event's reference, the CONTINUE arm chains
`MOVE_STEP` hops, supersession still kills stale chains, and `move_intents()` still
manufactures the speculation `MoveIntent` from the fanned seed. **Rejected**: deleting MOVE_TO
outright and seeding MOVE_STEP from the interaction arm — rebuilds speculation and
supersession for zero model gain.

## F4 — location `"target"` joins `"on"`; the MENU also enforces the location rule {#f4}

_2026-08-06._ `move_to` executes AGAINST a clicked tile the pawn is not standing on: its
location is `"target"` — the DESTINATION tile is the carrier the worker validates. Drink's
`"on"` is unchanged. The pie menu filters by the same rule: an `"on"` interaction is offered
only while the active pawn STANDS ON the clicked tile; a `"target"` one always (gates
permitting) — offering an option the worker would refuse is the drift class this repo names.
`unit.x/y/z` + "move to and execute" remain the recorded later generalization (interactions
F8).

## F5 — a RESERVED input vocabulary binds the menu; drink bakes its need {#f5}

_2026-08-06._ The menu is GENERIC: it can only fill inputs it understands. Input names become
a reserved vocabulary — `pawn` = the active selection's entity, `destination` = the clicked
tile's `position_reference`, `amount` = the carrier binding's magnitude (f32 bits). An
interaction whose signature uses only these is menu-composable; anything else refuses at the
menu (not offered). CONSEQUENCE: drink re-authors to `inputs = ["pawn", "amount"]` with
`need = "thirst"` BAKED into its satisfy effect — a generic menu cannot guess a need input,
and the F5 operand machinery (interactions stream) already supports baked names. The npc
composes the same two-input event. **Flagged to the user in the plan review** — this revises
the drink signature they authored in interactions F4.

## F6 — the `move` effect: `move = { target = "@pawn", to = "@destination" }` {#f6}

_2026-08-06._ The third effect type beside `satisfy`/`grant`. The worker resolves it exactly
like satisfy's operands (inputs or baked), validates, and queues the chain seed. An
interaction may author `move` OR `satisfy` (or both later); the worker's "nothing to do"
refusal generalizes from "no satisfy" to "no effect".

## F7 — the pie menu is a DOM overlay; an empty option set shows NOTHING {#f7}

_2026-08-06._ The menu renders as position-fixed DOM rects (the ConditionCards pattern — same
host, `zIndex` above the panel band, pointer events ON), anchored at the click's client x/y,
options at a fixed radius starting at 12 o'clock, equal angular spacing. No active selection,
a non-pawn selection, or zero available interactions → NO menu (not an empty ring). Rejected:
in-canvas rendering — the panel stack is already DOM, and a menu is UI chrome, not world.

## F8 — ground_speed is THE speed everywhere; the `speed` field dies {#f8}

_2026-08-06 (stat-model F12's planned handoff)._ The worker's chain spacing, the npc's trip
deadlines, and the client's speculation (`MoverLayer.speedFor`) all move to the DERIVED
`ground_speed` — pawn rows + corpus through the ONE `stat_eval` (a wasm accessor carries it to
TS). DELETED with the rewire: the `speed` field, `thing_speed`/`thing_speeds`/`thingSpeed`,
the worker's speeds table + `tics_for`, and stat-model's I10 guard test. A pawn with no
`walks` derives 0 → `can_move_ground` fails → it cannot move, which is CORRECT (that is what
the predicate is for).

## F9 — walls don't carry `move_to`: soft passability by authoring {#f9}

_2026-08-06._ grass/dirt/sand/stone/water carry `move_to`; `wall_smooth` does not — the menu
never offers Move To onto a wall and the worker refuses it (the tile doesn't offer the
interaction). This is NOT a passability system (the user deferred that explicitly in
interactions F8) — pathing still walks wherever the chain steps — but the carrier list is the
natural seam a real passability model later inherits.

## F10 — the legacy `wildlife` brain is DELETED {#f10}

_2026-08-06._ wildlife drives CLIENT-MINTED wolves (`wolf_key` raw ids) through
`move_entity` — the exact door this stream closes — and its own header says it is "kept as the
harness's regression fixture until the wolves-brain fully lands". It landed two streams ago.
Delete-don't-deprecate: porting it to interactions would preserve a fixture whose behaviors
(client-minted ids, random-cell spam) the model has outgrown.
