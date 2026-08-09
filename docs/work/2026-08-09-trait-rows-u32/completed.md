# Completed — trait-rows-u32

## 2026-08-09 — P3/P4 COMPLETE: ACTIVATE_TRAIT + sprint, MEASURED

**The verb** — `ACTIVATE_TRAIT = 19` (client-open): the worker validates the carrier BINDS
the trait (things via thing_traits, player-pawns via their BRAIN def's binds), the category
is `*_active`, and every `blocked_by` condition is inactive, then queues the def's authored
grants. The ritual bit THREE stale binaries in sequence (event-shard wasm, the edge, the
npc — each refusing verb 19 or the `activate` fields until rebuilt: I1's cliff, live).
**The content** — `sprint` (grants `sprinting` 60 tics, ground_speed −6 min 4, motivated+3;
`sprint_cooldown` 300 tics, ground_speed +4, uncomfortable+2 — the exhaustion), gated
`blocked_by = [sprint_cooldown]`; `rally` = the player-lane twin on the wolf_pack brain.
The six categories seed the registry (the master's category loop extended — sprint at
0x80090010, rally at 0x800C0010, exactly the packed derivation).
**The drills, MEASURED not eyeballed**: activation landed BOTH grants in the wolf's payload
(remaining 60/300 at the written tic) and `rallied` (120) on the player-pawn 0x40800001 —
the player lane works end to end; re-activation 19 tics into the cooldown REFUSED ("blocked
by an active condition"), and the accidental slow drill proved the re-arm (a retry 7 tics
AFTER expiry succeeded). THE SPEED, from entity_state_log positions: sprinting ≈ 7.0
tics/tile (12 baseline; authored 6 + trip overhead), winded EXACTLY 16.0 tics/tile
(12 + 4) — the stat modifiers provably reach the worker's hop pacing (I6's lie-detector
passed with numbers). The on-camera half stands as the world rendering + the wolf's panel
cards (capture u64rows); the quantitative read supersedes pixel-watching for pace.

## 2026-08-09 — P2 COMPLETE: every consumer on the ONE u64 row, LIVE

**The wire**: SET_NEED/GRANT_CONDITION arities widen to (obj, reference, data) — the u64 row
rides the u32-word program SPLIT; the worker's relays re-join with pack_row; every queue
site (interaction effects, inventory free-counts, restamp mirrors, the npc's drills and the
host's bookkeeping) authors the two-word form; event_shard republished (its wasm validates
arity). **The consumers**: worker (mint_sidecars by category+variant, the sweep/crossing
lanes keyed by FULL reference), npc (both brains' row maps re-keyed), edge (the mint's
full_rows → u64; the Need frame field u64 — ONE lossless number under the law), client/core
(api/protocol/engine/web), wasm (needs arrays cross as f64s with the row-law debug assert),
webgl (the ONE TS row-key site made lawful: modulo, never bitwise — the I9 comment teaches
at the site). **Verified LIVE**: registry re-seeded 234 defs CLEAN; pawn needs rows stored
as u64 (0x8000_80010030); the host's whole stage-1 loop runs on the new law (count 3 written
through the split wire → stored 0x3000_80010060 → fanned back); the world renders; a live
wolf's panel shows its condition cards evaluated from u64 rows (capture u64rows). The
modules item's hand-write drill = the count writes traveling the full event system.

## 2026-08-09 — P1 complete + the content re-author: EVERYTHING GREEN in shared/content

The corpus re-authored in the SAME commit as the tests (I3's one-commit law):
interactions.toml's `[[trait]]` defs → `[[pawn_trait_passive]]` (walks, corpus, bio, tags,
diets, forager, inventory, lumberjack) and `[[pawn_trait_constant]]` (emit_light);
players.toml's → `[[player_trait_constant]]`; every bind `level = N` → `variant = N-1`
(0-based F6), `constant = true` DELETED corpus-wide (category is the flag now); brains.toml
rides along. Every inline test fixture in all five source/test files swept likewise; the
affordance tag-check scans the six lanes; the dead "level 0 = absent" defensive test arm
REMOVED with its epitaph. Verified: 70 lib + 2 golden (re-blessed DELIBERATELY — the
category move is the diff) + 2 drill tests green — the whole crate loads, evaluates, and
pins on u64 rows with variant tiers. REGISTRY NOTE for the sweep's restart: the re-homed
defs get NEW registry rows under their new categories (append-only; the retired rows
remain); the master seed must come up ZERO-divergence.

## 2026-08-09 — P1 (superseded, third piece): shared/content's LIB on u64 rows

The loader model restructured: `TraitBind {name, variant}` (level + the constant flag DEAD);
`Bundle.trait_defs: Vec<(name, params, category 8..13)>` replaces the traits/player_traits
pair (ONE namespace, params-by-name/by-ref across all six categories, `trait_category()`);
the six `[[…]]` tables parse through the ONE conversion; `[[trait]]`/`[[player_trait]]`
REFUSE with the migration message; binds parse `variant` (0-BASED tiers — F6 refined);
thing/brain validation is category-law (non-pawn things = constant categories only; brains =
player_* only; the ACTIVE ≤3 LOAD DOOR lands early — F3/I5). The merged accessor +
object_lights + the THREE evals (needs/stat/emotion) consume u64 rows: reference =
`row_reference` (reconstruction DELETED), tier = the reference's variant, value/remaining =
`row_data`. THE LIB COMPILES CLEAN. STILL RED (the continuation's order): the crate TESTS
(64 errors — pack_gameplay_row/level pins) + the CONTENT re-author under the six categories
(the corpus still authors the retired tables) move TOGETHER, then goldens re-bless, then the
consumer sweep (worker → npc → wasm/webgl → edge mint's brain-bind reads) + restarts, then
ACTIVATE_TRAIT (P3) and sprint (P4).

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
