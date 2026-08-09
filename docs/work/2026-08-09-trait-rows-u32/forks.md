# Forks — trait-rows-u32

## F1 — trait rows are BARE u32 references; condition/need rows are u64 (REVISED, user)

REVISED TWICE at review (the level census, then the standardization). LEVEL IS REMOVED —
a trait's tier is its definition's VARIANT nibble (u4, 15 tiers + the 0 default — five
times today's authored depth). A stored TRAIT row is JUST the full u32
`definition_reference`: no data lane, no packing helper, payload TRAIT entries stay ONE
word, `object_trait_rows` returns plain refs. Condition and need rows STANDARDIZE on u64 =
`definition_reference:32 | data:32` — full subtype AND variant lanes on all three families,
and the data lane GROWS to u32 because "needs and conditions should likely use more data"
(the user): the def's TOML declares how its u32 is interpreted (F7). Their payload entries
and the `needs` sub-table columns widen. `pack_gameplay_row`'s trait callers DELETE; its
condition/need callers move to the u64 shape — either way every consumer breaks loudly at
compile, which is the sweep finding itself. Rejected: a uniform u64 for traits too (a data
lane nothing fills); keeping level beside variant (two tier numbers is how drift starts);
u16 data held (the standardization + headroom beat the two saved bytes).

## F6 — level → variant: the authoring and eval mapping

The per-level ARRAYS stay as authoring sugar — array index i authors VARIANT i+1 (`walks`'s
`add = [16, 12, 8]` = variants 1..3), so today's corpus re-authors by RENAMING `level = 2`
binds to `variant = 2` and nothing renumbers. Marker traits (bite, herbivore…) live at
variant 0 — the bare def IS the capability. `trait_params(name, level)` lookups become
params-by-REF (the variant indexes the table); "level 0 = absent" semantics die — absence
is absence of the row. The registry already numbers per-variant tuples (the u4 slot law);
each authored tier gets its registry row like any variant.

## F7 — the data u32 is DEF-INTERPRETED: the TOML declares the encoding

"The toml can determine HOW we interpret the u32 available" (user). A condition/need def
authors its data ENCODING — the default shapes are what exists today (need: the u16
fixed-point value in the low half; condition: the u16 remaining-at-write in the low half),
and a def may instead declare LANES, e.g. a condition packing `4 × i8` bound to authored
targets (emotion nudges, stacked magnitudes, charge counts). The ONE eval reads data
THROUGH the def's declared encoding — consumers never hard-code a layout again; new
encodings are pure content + one decoder arm. v1 ships the two default encodings plus the
`4 × i8` lane form (sprint/exhaustion don't need it, but the drill pins the machinery);
richer encodings are appends. This keeps the system compact and generic — the row is always
(ref, u32), the MEANING lives in the corpus.

## F2 — six authored categories; the old two retire empty

`pawn_trait_{constant,active,passive}` + `player_trait_{constant,active,passive}` APPEND to
the gameplay palette (order law: at the END). The corpus re-authors every existing trait
under its home: `walks`, `corpus`, `biological_lifeform`, tag-carriers (`bite`) →
`pawn_trait_passive`; `emit_light` carriers (torch's constant) → `pawn_trait_constant`;
`wolf_pack`/`bunny_fluffle`/`area_of_influence` → `player_trait_constant`. The retired
`trait`/`player_trait` categories keep their registry rows (append-only) but author NOTHING;
the loader keeps parsing `[[trait]]` only long enough to refuse it with a migration message.
The CONSTANT-assignment law (trait-lights) becomes structural: a `*_constant` def's binds
are constant BY CATEGORY — the per-bind `constant = true` flag retires with its ambiguity.

## F3 — activation is a worker-validated intent; slots are a load + run law

A new client-open verb `ACTIVATE_TRAIT` (obj, full u32 trait reference): the worker validates
the carrier BINDS the trait (active category), the slot law (≤ 3 active binds — also refused
at LOAD), and the trait's availability predicate (sprint: no `sprint_cooldown` row), then
executes the trait's authored `activate` block — a list of condition GRANTS (name + duration
+ magnitude), reusing GRANT_CONDITION wholesale. Rejected: activation as an interaction
(interactions are world-verb shaped — actor/target/adjacency; an ability toggle has no
target and would drag the intent-queue walk-then-act machinery in for nothing); client-side
grant composition (the worker owns validation — spawn-authority's lesson).

## F4 — sprint is two simultaneous grants; the cooldown IS the gate and the cost

Activation grants `sprinting` (duration S; `stats = [{ stat = "ground_speed", … }]` through
the ONE combiner — movement pacing reads the derived stat, so the speedup applies from the
next hop) and `sprint_cooldown` (duration S + R, granted the SAME tic): its presence is the
re-activation refusal (F3's predicate) and its own modifiers carry the exhaustion (need/stat
penalties over the recovery window). No on-expiry chaining, no new condition kinds — the
overlap of two timers expresses the whole lifecycle. Rejected: expiry-triggered grants (new
scheduler machinery for something two durations already say).

## F5 — conditions and needs take the same row shape in the same stream

Conditions' u32 rows have the identical category-blindness; the u64 shape lands for ALL
gameplay rows at once — a split migration would leave the eval reading two shapes forever.
Need refs in verbs (SET_NEED/RESTAMP_NEED operands) already carry full u32 references —
those verbs' wire shapes are UNCHANGED; only stored rows and payload entries widen.
