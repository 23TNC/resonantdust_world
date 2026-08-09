# Forks — trait-rows-u32

## F1 — trait rows are BARE u32 references; condition/need rows are u64 (REVISED, user)

REVISED at review after the level census (14 traits, deepest table 3 levels, highest bind
L2): LEVEL IS REMOVED — a trait's tier is its definition's VARIANT nibble (u4, 15 tiers +
the 0 default — five times today's depth). A stored TRAIT row is therefore JUST the full
u32 `definition_reference`: no data lane, no packing helper, payload TRAIT entries stay ONE
word, and `object_trait_rows` returns plain refs. Conditions and needs genuinely use their
u16 data (remaining_at_write / the fixed-point value), so THEIR stored rows are u64 —
`reserved:16 | data:16 | reference:32` (reserved ZERO, I9) — and their payload/sub-table
lanes widen. `pack_gameplay_row`'s trait callers DELETE; its condition/need callers move to
the u64 shape — either way every consumer breaks loudly at compile, which is the sweep
finding itself. Rejected: a uniform u64 for traits too (a data lane nothing fills);
keeping level beside variant (two tier numbers is how drift starts).

## F6 — level → variant: the authoring and eval mapping

The per-level ARRAYS stay as authoring sugar — array index i authors VARIANT i+1 (`walks`'s
`add = [16, 12, 8]` = variants 1..3), so today's corpus re-authors by RENAMING `level = 2`
binds to `variant = 2` and nothing renumbers. Marker traits (bite, herbivore…) live at
variant 0 — the bare def IS the capability. `trait_params(name, level)` lookups become
params-by-REF (the variant indexes the table); "level 0 = absent" semantics die — absence
is absence of the row. The registry already numbers per-variant tuples (the u4 slot law);
each authored tier gets its registry row like any variant.

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
