# Completed — input-rework

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
