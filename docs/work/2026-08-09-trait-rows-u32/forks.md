# Forks — trait-rows-u32

## F1 — ONE u64 row shape: `data:16 | definition_reference:32`

The user's "hold as u32" names the IDENTITY: the full `definition_reference` (its u12 subtype
is the point). The data lane (trait level / condition remaining / need value) still needs its
u16, so the STORED row is u64 — `reserved:16 | data:16 | reference:32`. One shape for all
three families keeps the ONE-eval law one law. Consequences, taken deliberately: payload
TRAIT/CONDITION entries carry 2 operand words instead of 1 (the opcode stream's count field
already supports it); the `needs` (and player_pawn `needs`) sub-table column widens u32→u64
(a module schema change — the wiping-republish ritual); `pack_gameplay_row`/`gameplay_row_*`
helpers change signature so every consumer breaks LOUDLY at compile, which is the sweep
finding itself. Rejected: paired u32 words (two lanes to keep in sync); keeping u32 rows with
a category nibble squeezed in (the subtype is 12 bits — it does not fit; half-measures are
how the current collision happened).

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
