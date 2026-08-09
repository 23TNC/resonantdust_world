# Completed — trait-rows-u32

## 2026-08-09 — P1 (second piece, IN FLIGHT): the module lanes

Both shards' `needs.need` columns are u64 (set_need/grant_condition/spawn take u64 rows;
the uid still keys the low 16); published as in-place updates — the column change WIPED the
needs rows (I2's posture; the sweep's restarts re-mint). st-bindings + edge bindings carry
`need: u64`. THE STACK IS MID-MIGRATION: the running worker/edge binaries still speak u32
rows and their reducer calls will FAIL against the new modules until the P2 sweep rebuilds
every consumer — the item's hand-SET_NEED drill runs AFTER the worker converts. NEXT (the
P2 sweep, in order): shared/content (the ONE eval on u64 rows; the loader's six category
tables + level→variant per F6; the encoding seam per F7) → worker → npc → wasm/webgl →
edge → content re-authored under the six categories → goldens/pins re-blessed → restarts →
the drill.

## 2026-08-09 — P1 (first piece): the codec core

`pack_row(reference, data) → u64` + `row_reference`/`row_data`/`row_lanes_i8x2` (F7's 2×i8
read) + `row_law_ok` (the dead-16 assert); `pack_gameplay_row`/`gameplay_row_key`/
`gameplay_row_data`/`gameplay_row_reference` DELETED (the reconstruction is obsolete — the
row carries its full reference). Payload TRAIT entries = `[header(2), reference, data]`,
CONDITION = `[header(3), reference, data, written_tic]` — u64 rows ride the u32-word stream
SPLIT (the word-lane law); upserts key on the FULL reference (a TIER change is a different
def — remove+add; the data lane rewrites in place). The six categories appended (palette
13, the frozen-order test extended); `trait`(3)/`player_trait`(7) marked RETIRED EMPTY.
Verified: 72 codec tests green, including the new round-trip-under-the-law pin (lanes
(-2,5) from 0x05FE; a row daring the dead 16 fails) and the retired-opcode guard skipping
old-shape entries whole (count mismatch = no row — the wipe posture's last line).

## 2026-08-09 — P0: the paper

VARIABLES.md rewritten at the three seats: **the ONE u64 row** (dead:16 | data:16 |
reference:32) with THE 48-BIT TRANSPORT LAW stated inline (dead asserted zero; TS splits by
division, never `>>> 32`); **level DELETED** — tier = the variant nibble (census in the
stream folder: deepest table 3, highest bind 2), per-level arrays kept as authoring sugar
(index i → variant i+1), "level 0 = absent" dead; **the data u16 def-interpreted** (F7 —
defaults value/remaining, trait ZERO-reserved, 2×i8 lanes declarable, unknown REFUSES);
**the six categories** with constant-BY-CATEGORY superseding trait-lights' per-bind flag
(the old categories retired EMPTY, their laws — things constant-only, the merged accessor,
constant-wins, verb refusal — carried forward intact); **the ACTIVE law** (3 slots, two
doors, ACTIVATE_TRAIT worker-validated, sprint as the canon: two overlapping grants, no
expiry chaining). docs-check green.
