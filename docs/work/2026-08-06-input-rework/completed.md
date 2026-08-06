# Completed — input-rework

## 2026-08-06 · P1 — loader + corpus (3/3)

**Loader**: `InteractionParams` gains `menu_text` (default = label) and
`move_effect: Option<MoveEffect>` (`{ target, to }`, `@ref`s resolved at load exactly like
satisfy's); the location set is `{"on","target"}`; an interaction with NEITHER effect
refuses (`authors no effect`). Fixture fallout handled: the location-refusal assertion moved
to the new message, and stat_eval's corpus fixture gained a real satisfy (it authored an
effect-less drink). 39/39 lib tests, with `the_input_rework_fields_round_trip` pinning the
move_to shape verbatim. **Corpus**: `move_to` authored (menu_text "Move To",
`can_move_ground`, `["pawn","destination"]`, the move effect, location "target");
grass/dirt/sand/stone/water carry it (a magnitude-less bind — `("move_to", 0.0)`),
`wall_smooth` deliberately does NOT (F9); drink re-authored to the F5 two-input baked-need
signature with `menu_text = "Drink"`. `rd content-check` clean. **Golden**: 9+/6−, every
line reviewed — move_to's params + its seed ref `0x80040020`, drink's
`satisfy.need = Name("thirst")`, menu_text on both rows, the five carrier lists, and
`wall_smooth []` byte-checked. The 2-pass gate: native 39 + golden 3, wasm32 pkg rebuilt.
NOT yet touched (P2/P3's turn): the worker still lacks a move arm, npc/client still compose
raw MOVE_TO, and `speed = 12` still stands (its deletion rides P3 with the consumers).

## 2026-08-06 · P0 — the contract, documented before built (3/3)

**VARIABLES.md** — the schema block gained the full `[[interaction]] move_to` sample
(menu_text, `can_move_ground`, `inputs = ["pawn","destination"]`, the `move` effect, location
`"target"` with the menu-enforces-it note) beside drink's re-authored two-input signature
(need BAKED — F5's reserved vocabulary documented at the `inputs` field); the wolf sample's
`speed = 12` is GONE (replaced by the derived-`ground_speed` note) and the water carrier
sample shows `{ name = "move_to" }` with the walls-don't-carry-it rule (F9). **ACTIONS.md** —
`MOVE_TO`'s row marks it WORKER-ONLY (F3) with the interaction arm as its only composer;
§Movement opens with the new front door (`EXECUTE_INTERACTION(move_to)` → validation → the
seed at `master+4`), chain spacing reads the DERIVED `ground_speed` (the `speed` field
declared deleted), the trip-serial note names the worker-queued event's reference (I2), and
the speculation paragraph pins the `MoveIntent` continuity claim (I1) — also fixed in
passing: the §chain bullet still said the continuation was "`MOVE_TO obj dest`" (it has been
`MOVE_STEP` since movement-hardening). **design/input-model.md** — the button table (right =
active selection, left = pie menu, build mode untouched), menu content/composition/
presentation/lifecycle, the empty-set rule, and the non-goals, with the user's contract
quoted verbatim. Verified: `bin/rd docs-check` green (506 files).
