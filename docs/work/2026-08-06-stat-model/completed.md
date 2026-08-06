# Completed — stat-model

## 2026-08-06 · P0 — the schema, documented before parsed (4/4)

The three authoritative homes took the model BEFORE any code. **VARIABLES.md** — §Needs &
conditions became **§Pawn gameplay state**: the four families, the packed row
(`data:16 | kind:12 | variant:4`, low 16 ≡ the def ref's low 16 so the full reference
reconstructs as `TYPE_GAMEPLAY<<28 | subtype<<16 | low16`), the u16 fixed-point law (encoding
domain = AUTHORED bounds, always — modifiers narrow the clamp, never re-scale the encoding),
the combiner (adds SUM · ranges INTERSECT max-of-mins/min-of-maxes · empty intersection →
authored `winner` · rate multipliers form a PRODUCT), the re-stamp law, and the worked quenched
window (`39.9994 − 8.3333 − 6.4815 ≈ 25.1846` at tic 6000) that P1's eval test must reproduce.
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
