# Completed — interactions

## 2026-08-06 · P1 + P2 — the loader, the registry, the corpus (6/6 + 3/3)

One build unit by necessity: removing `id` from the needs schema makes the old corpus refuse
(`deny_unknown_fields`), so loader and corpus moved together.

**Codec first**: `TYPE_GAMEPLAY = 8` + the code-owned category palette (`GAMEPLAY_CATEGORIES`,
one spelling shared by loader and allocator — categories are schema vocabulary like types, NOT
content like species, or F16 would have owned them); `payload.rs` reshaped to the TABLES.md
entries — NEED count 3 (`def_ref · f32 bits · set_tic`), CONDITION count 2 — with a
layout-version guard: an old-count entry is SKIPPED whole (the re-mint posture, [I1](issues.md#i1)),
proven by a test that feeds the old packed word and watches it be ignored, not misread. 64 tests.

**Loader**: the five gameplay categories parse with derived taxonomy ([F9](forks.md#f9)) and
name-uniqueness per category; `NeedParams` gains `min`/`max` (inverted domain refuses); [F5](forks.md#f5)
effects resolve `@refs` to input INDICES at load (dangling → refusal), unknown
grants/traits/interactions/affordances refuse; carriers bind `(name, magnitude)`. Resolution is
registry-first with the corpus-position SEED fallback (`gameplay_reference`/`gameplay_lookup` +
`with_gameplay_registry`), mirroring the kind-seed posture. `needs_eval` moved to
`(u32 ref, f32, u16)` rows on authored domains — the never-crossed clamp guard generalised
`<= 0` → `<= min`. 26 lib tests + a new 0..100-domain test (incl. deficit clamp).

**Golden** ([I8](issues.md#i8) discipline): dump extended (three registries + params, seed
refs, carrier tables), re-blessed at 48+/7−, EVERY line reviewed: the wolf's needs-table slot
1.0 → `0x80010010`, bands to raw units, probe condition ids → `0x8002_00{1,2,3}0`, and the mid
probe's next-crossing 3262→3241 explained exactly (25.0 replaces the old q64 = 64/255 — probe
quantization, not eval drift; the "full" probe is bit-identical at 14041).

**Registry live**: the master's allocator (it composes, the index records — I8) gained the
gameplay arms; `bin/sim test master` 6/6 including allocator≡seed agreement on drink/thirst/
drink_water; restarted master seeded 189 defs and `/definitions` serves the 7 gameplay rows,
`drink = 0x80040010`.

**Corpus**: `interactions.toml` authored (model in comments); `needs.toml` re-authored — no
ids, thirst `0..100`, bands `10/35`/`0/10`; wolf gains the trait, water the `drink_water 3`
binding. `/content` ships interactions.toml by construction (the block-based F2 filter) and
still withholds biomes — verified live. `rd content-check` clean.

**Wasm**: `conditionLabels` (positional) DELETED for `conditionLabelOf(ref)` — the panel's one
line updated; `withGameplayRegistry` added for the client's registry binding. wasm32 pkg
rebuilt green = the 2-pass gate's second pass.

NOT yet touched (P3's turn): the pawn module's reducers still compose old-shape words, and npc
still speaks the old eval tuples — the worker/npc/edge binaries redeploy with P3.

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
