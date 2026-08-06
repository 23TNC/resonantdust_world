# Completed — interactions

## 2026-08-06 · P0 — the schema, documented before parsed (3/3)

The three authoritative homes took their halves BEFORE any code: **VARIABLES.md** — the gameplay
taxonomy (`type = "gameplay"`, category subtypes, kind, variant `default`; registry-numbered, no
authored ids) with [F9](forks.md#f9) resolved en route (the taxonomy DERIVES from the category
table — authoring `subType = ["trait"]` inside `[[trait]]` is a drift lane, not a freedom), the
f32 value domain (need-authored `min..max`, the `clamp(sat + 3.0, 0, 100)` worked example), and
the full `[[need]]`/`[[condition]]`/`[[trait]]`/`[[interaction]]`/`[[affordance]]` + carrier
`affordances` spellings; **TABLES.md §payload** — NEED grows to count 3 (`def_ref · f32 bits ·
set_tic`), CONDITION to count 2 (`def_ref · grant_tic`), opcode VALUES unchanged, with the
explicit not-read-compatible note (dev pawns re-mint, [I1](issues.md#i1)); **ACTIONS.md** —
`EXECUTE_INTERACTION = 12` (variable arity, the user's F4 layout verbatim) as a
**BUILD_WALL-pattern verb**: it writes NOTHING itself and the worker queues `PROMOTE SET_NEED` /
`PROMOTE GRANT_CONDITION`, so the real write set rides typed verbs and grouping never has to
route untyped input words; SET_NEED/GRANT_CONDITION operand meanings amended (u32 gameplay def
refs, f32 satisfaction bits). Also fixed in passing: the schema's wolf sample still authored
`id = 7` (a pre-registry leftover). Verified: `bin/rd docs-check` green.
