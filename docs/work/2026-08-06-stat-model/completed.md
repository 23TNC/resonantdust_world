# Completed — stat-model

## 2026-08-06 · P3 + P4 — the tables, the worker, the fan, the drinking wolf (4/4 + 3/3)

**The pawn shard**: the `needs` table (uid = `entity:32|key:16`, zone-slaved alongside
`payload` by the state hook) + `spawn(needs)`; `set_need` arity 5→4 (ONE packed row),
`grant_condition` packs remaining|key. The RE-STAMP duty moved to COMPOSERS
([I12](issues.md#i12), found designing this phase: the module holds no corpus, so P0's
"reducer re-stamps" claim was wrong — ACTIONS/TABLES amended) and the EVAL hardened for the
derivable gap: a condition row written AFTER the need's stamp gets a rate window STARTING at
that offset (`RateWindow.start`), so a lone first grant evaluates exactly with no re-stamp.
`need_bounds` gives the write-side effective clamp. The worker composes both mint sidecars
(`mint_sidecars` — F11), gates on `interaction_available` (the SAME fn the npc asks), reads
needs from the sub-table, quantizes ONCE at write, and queues re-stamping SET_NEEDs beside
any grant whose condition modifies other needs. Bindings regenerated (edge + st-bindings).

**The fan** ([I8](issues.md#i8)): `Need` wire frame (edge per-zone sub + snapshot replay +
insert/update relays) → `Event::PawnNeed` (both engines + wasm marshal) → npc buffer +
MoverLayer `pawnNeeds` (stride-2). The dev registry was RESET by the full-module republish
(`rd redeploy --run` — codec in every closure), which also cleanly retired the dead
`drink_water @ 0x80050010` row: `ensure_definition` REJECTS collisions, so the re-seed was
the correct lane; master seeded **193** definitions (189 − 7 old gameplay + 11 new).

**The drills, all seeds stated**: fresh wolf `0x30800000` minted with thirst `0xFFFF_0010`
@918 + payload `[TRAIT bio@1][TRAIT walks@2]` (sql). Off-water `NPC_INTERACT=drink` →
`interaction dropped … the tile does not offer this interaction (F8: on-tile only)`, zero
splices. `NPC_THIRST=12 NPC_HOME=100,62` → Thirsty@1436 → walk to (102,68) → sips of
EXACTLY +3.0 with the quenched ×0.5 rate VISIBLE between them (0.0138 per 6 tics =
6·100/43200) → band cleared at 35.17 → `["quenched"] mood 0.7` → the thermostat dipped back
at 1718 exactly as the piecewise crossing (1713+eval cadence) predicted, and the wolf
re-sipped. Grant drill: quenched `3600@3213` + thirst re-stamped at the SAME tic (sql) —
found+fixed: the combined seed+grant drill raced (re-stamp composed the pre-seed value), so
the grant lane defers 3 s past init; re-run holds 49.97%. HONEST GAP: the predicate
NEGATIVE case is undrillable (every pawn kind carries biological_lifeform); the gate runs
positively on every sip through the one shared fn.

**The panel** (browser, `:5174/?focus=102,68`): wolf `0x30800001` mood 70% **Quenched
+0.20 3170t**; the thermostat window mood 55% **Thirsty −0.15 + Quenched +0.20 2433t** in
authored priority order — rendered ENTIRELY from the fanned payload + needs rows through
the wasm eval (`pawnConditions(payload, needs, now)`).

## 2026-08-06 · P1 + P2 — codec, the shared evals, the corpus (6/6 + 2/2)

One build unit again by necessity: the schema change makes the old corpus refuse
(`deny_unknown_fields` on `affordances`/`requires`/`variants`), so loader and corpus moved
together, exactly as the interactions stream's P1+P2 did.

**Codec** (71/71 tests): `pack_gameplay_row`/`gameplay_row_key`/`_data`/`_reference` — the
low 16 ≡ the def ref's low 16, proven by a reconstruction test; new `value.rs` holds THE
quantize/dequantize pair (half-away-from-zero, defined once; the P0 window's `40.0 → 26214 →
40.0000` pinned — which CORRECTED P0's write-up, the first draft claimed 39.9994; re-stamp
stability test: re-quantizing a dequantized value is the identity); `GAMEPLAY_CATEGORIES`
appends `stat` (id 6) with the append-only law in a freezing test. **Payload**: values 2
(NEED) and 3 (old CONDITION) RETIRED — a reshape retires its value and claims a new one, so
old rows read as unknown entries; CONDITION=4 (`remaining_at_write:16|key` + written_tic),
TRAIT=5 (`level:16|key`); upserts match the row's LOW 16 so a re-grant carrying different
remaining still refreshes in place. SET_NEED's signature dropped to `&[Write, Imm]`.

**Loader**: all six categories; traits carry per-LEVEL modifier tables (authored as
per-field arrays that must agree on length); conditions carry scalar `stats`/`needs`
modifiers; affordances are `check = { stat, above|below }` (exactly one, stat must exist);
interactions list `affordances`; carriers bind `interactions = [{name, magnitude}]`; things
bind `traits` with levels (bare string = 1, range-checked against the trait's table).
**[F13](forks.md#f13)** resolved en route: a DERIVED condition may not modify needs — its
liveness is computed FROM need evaluation, so the modifier would feed the thing that decides
it; the loader refuses `duration = 0` + `needs = [...]`.

**The shared evals**: new `stat_eval.rs` — `combine_bounds` (the F6 combiner; the user's
cases pinned verbatim: 3..7 ∧ 4..8 → 4..7, 2..3 ∧ 4..5 → winner pins 4 or 3),
`stat_value` (Σ adds clamped; walks level 2 → 12), `affordance_passes`,
`interaction_available` (the ONE availability question), `condition_remaining` (F3 with the
future-stamp guard). `needs_eval.rs` reworked to packed rows: `rate_windows` (trait windows
unbounded; stored-condition windows cut at remaining-at-set — the re-stamp law means no
window ever STARTS mid-row), piecewise `depletion` + `crossing_elapsed`; the P0 worked
window reproduces (`25.1852` at tic 6000), and a piecewise-crossing test proves the wake
order (quenched expiry at elapsed 100, band crossing at 101). 37/37 lib tests.

**Corpus**: `interactions.toml` re-authored (stats ground_speed 0..240 + metabolism 0..10,
walks `add = [24, 12, 6]`, biological_lifeform → +1 metabolism, can_drink/can_move_ground
predicates, drink gated on can_drink); `needs.toml` — quenched gains the thirst ×0.5 rate
modifier (the living I4 proof case); water binds `interactions = [{drink, 3}]`; wolf binds
`["biological_lifeform", { name = "walks", level = 2 }]`. **Walks re-based** from the
user's 60/50/40 illustration to 24/12/6: the I10 guard (derived ground_speed == the `speed`
field, now a golden-suite test deriving 12.0) cannot hold otherwise — noted under
[F12](forks.md#f12)'s handoff. `rd content-check` clean.

**Golden**: 44+/19−, EVERY line reviewed — new fields (`min_wins`, empty `stats`/`needs` on
old conditions, `affordances` on drink), quenched's NeedModifier, walks' three levels, the
predicate affordance params (drink_water DELETED, can_drink/can_move_ground in), stat
params, seed refs `walks 0x80030020 · can_drink 0x80050010 · can_move_ground 0x80050020 ·
ground_speed 0x80060010 · metabolism 0x80060020`, carrier renames, and the needs probes
BIT-STABLE (the probe values quantize exactly). Noted for P3: the LIVE registry still holds
`drink_water @ 0x80050010` — the dev posture is a registry re-seed with the module
republish (re-mint world); a prod world would keep the dead row and allocate fresh ids,
with registry-first resolution carrying the difference.

**The 2-pass gate**: native (codec 71, content 37 + golden 3) + wasm32 (`rd build shared`
green — `pawnConditions`/`pawnMood`/`pawnNextCrossing` take the stride-2 needs rows now;
the TS callers move in P4).

## 2026-08-06 · P0 — the schema, documented before parsed (4/4)

The three authoritative homes took the model BEFORE any code. **VARIABLES.md** — §Needs &
conditions became **§Pawn gameplay state**: the four families, the packed row
(`data:16 | kind:12 | variant:4`, low 16 ≡ the def ref's low 16 so the full reference
reconstructs as `TYPE_GAMEPLAY<<28 | subtype<<16 | low16`), the u16 fixed-point law (encoding
domain = AUTHORED bounds, always — modifiers narrow the clamp, never re-scale the encoding),
the combiner (adds SUM · ranges INTERSECT max-of-mins/min-of-maxes · empty intersection →
authored `winner` · rate multipliers form a PRODUCT), the re-stamp law, and the worked quenched
window (`40.0 − 8.3333 − 6.4815 ≈ 25.1852` at tic 6000; the 40.0 stamp is EXACT — 26214 is
40% of 65535) that P1's eval test must reproduce (corrected in P1: the first write-up claimed
a 39.9994 dequantize).
The TOML schema block rewrote all six categories with `[[stat]]` new; walks authors
`add = [24, 12, 6]` — the user's 60/50/40 illustration re-based so the wolf's level-2 value
EQUALS its `speed = 12` (the [I10](issues.md#i10) guard demands they cannot drift; noted under
[F12](forks.md#f12)). **TABLES.md** — opcode policy hardened to RETIRE-on-reshape (NEED 2 and
CONDITION 3 are dead values; CONDITION re-lands as 4, TRAIT as 5), and the `needs` sub-table is
specified (uid = `entity:32|need_key:16`, zone-slaved like payload, lazily-evaluated
`(value, set_tic)`, deliberately no log twin — the churn is what was evicted). **ACTIONS.md** —
SET_NEED arity 3→2 (packed row, ONE quantization by the composer), GRANT_CONDITION carries
`remaining_at_write` packed + the reducer's re-stamp duty, EXECUTE_INTERACTION's row now names
the predicate gate. Verified: `bin/rd docs-check` green (4 pre-existing warnings only).
