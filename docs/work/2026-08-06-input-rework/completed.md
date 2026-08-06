# Completed — input-rework

## 2026-08-06 · P5 — the verdict (2/2; 19/19 — the user's eyes close the stream)

The truth pass: the delete-greps run EMPTY (`move_to_program`/`moveEntity`/`moveSelf`/
`thing_speed(s)`/`thingSpeed`/`speed::resolve`/wildlife — three stale COMMENTS swept in
passing, plus the `Wildlife` login-name default); memories rewritten
(input-rework-delivered; ui-select-delivered's button contract marked superseded;
stat-model's F12 handoff marked closed; pawn-movement + automated-player rows updated).
The cold boot: edge re-exec + master/orchestrator/worker/npc bounced together — the
registry re-seeded 194 definitions (+`move_to`), the worker loaded `interactions=2`, the
npc adopted its wolf and drove trips through the interaction door only, and the
`NPC_THIRST=25` wolf ran THREE unprompted thermostat cycles (Thirsty → walk to (102,68) →
drink → Quenched; each queued drink flips the band within tics). On the reloaded browser:
a button-2 pointerdown selected the wolf (panel + outline) and the left-click water menu
opened with Move To. What remains is the user's look.

## 2026-08-06 · P2 + P3 + P4 — the door re-hung, the buttons swapped, the menu lives (4/4 + 3/3 + 4/4)

**The server** (P2): the worker's interaction arm generalized — the acting pawn from either
effect, the CARRIER tile by the location rule (`"on"` = under the pawn, `"target"` = the
move destination, per-biome merged read), satisfy scoped, the `move` effect queueing the
same `PROMOTE_EVENT PROMOTE MOVE_TO` seed the client used to send; chain spacing derives
`ground_speed` PER HOP (a mid-trip stat change slows the chain); the speeds table,
`tics_for`, and `load_corpus`'s speeds half deleted. The npc composes
`EXECUTE_INTERACTION(move_to)` for every trip, derives its deadlines, and the legacy
`wildlife` brain is gone (F10). LIVE: trips through the new door only, one MoveIntent per
trip (I1), a 7-hop trip in exactly 8×12 tics.

**The client** (P3): right click = the whole former selection behavior (verified live —
pawn silhouette + panel, tile box + title); speculation reads `pawnGroundSpeed` (the panel
shows the DERIVED `12 tics/tile`); `Command::Move`/`move_to_program`/`moveEntity`/
`moveSelf` deleted through core/wasm/TS; humans re-bound to walks level 1 (24 t/t — the
old `speed = 16` has no walks slot; an accepted pace change to a debug fixture).

**The menu** (P4): `PieMenu.ts` (rects at radius 72, equal angles from 12 o'clock, body-
hosted, z 60000, document-capture dismissal), `tileMenuOptions` as THE one availability
filter (predicates + location — fed by the bridge's autotile `tileKindAt` map via
`tileDefAt`), and the F5 vocabulary composer (`pawn`/`destination`/`amount` →
`composeInteraction`). LIVE, real mouse: water-not-standing = Move To only; standing =
Drink + Move To (captured, with the wolf Thirsty); Move To walked the wolf to (102,68)
with the glide; Drink sipped `18.48 → 21.48` + quenched; two rapid orders left ONE chain
(rest at the second dest — I2); npc + menu orders side by side (I10 as accepted).

**Found live, all fixed**: [I11](issues.md#i11) — the universal `move_to` carrier broke
the npc's drink filter (fired move_to through the 3-input drink composer; the drink pass
now filters to SATISFY carriers and `fire_interaction` binds by the vocabulary);
the pie menu rendered UNDER the canvas (panel z-bands reach 50k → menu at 60k) and
drifted off-cursor (#app is a transformed containing block → body-hosted); and the
FIRST edge-door drill FAILED OPEN — the redeploy's edge build hit the docker/WSL2 MTIME
MISS (the memory's exact failure: `Finished` with no `Compiling`; a raw MOVE_TO walked
the wolf toward nowhere) — `touch` + rebuild + re-exec, after which the raw verb is
rejected and the wolf stays put. The edge item ran AFTER P3's client swap per I3 (the
plan's in-P2 position was itself the I3 hazard).

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
