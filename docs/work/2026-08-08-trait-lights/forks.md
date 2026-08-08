# Forks — trait lights

_A choice I resolved, with what was rejected and why. A fork is mine; a
[blocker](blockers.md) is the user's._

## F1 — the keyword is `constant`, not `immutable` {#f1}
_2026-08-08 · raised by the user at plan time — theirs to overrule_

**Chosen.** `constant`.

**Why.** Immutable names only half the contract (cannot be modified). The
property doing the real work is stronger: the value is STATICALLY KNOWN FROM
THE DEFINITION — which is exactly what lets the assignment skip spacetime and
bake into cold. That is `const` semantics, not just write-protection: a
runtime-granted trait could be immutable-after-grant and would still need a
stored row. **Rejected — `immutable`**: truthful but weaker; it invites a
future "immutable but runtime-assigned" reading that reopens the storage
question this feature exists to close. (`innate` was the gameplay-flavored
third option — available if the user prefers flavor over precision.)

## F2 — `constant` is an ASSIGNMENT-SITE flag, not a trait-def property {#f2}
_2026-08-08 · resolved at plan time_

**Chosen.** The flag rides the TOML bind:
`traits = [{ name = "emit_light", level = 1, constant = true }]`.
The trait DEF stays neutral.

**Why.** The user's own sentence — "ANY trait can be added to things/pawns in
toml and marked constant" — names the assignment as the carrier. It also keeps
a light-carrying trait grantable non-constant on a pawn at runtime (a spell, a
buff) while the torch's copy is baked. **Rejected — a def-level
`constant = true`**: it would fork every trait into constant/non-constant twins
the moment one kind needs the other flavor.

## F3 — the u8|u8 level split is WITHDRAWN (user revision) {#f3}
_2026-08-08 · the user's call at plan review_

**Chosen.** NO codec change. The trait row keeps its u16 level exactly as
today; reach (and every other light variable) is HAND-AUTHORED on the trait
def's per-level `emit_light` table, so the row never needed a second byte.

**History, kept honest:** the first draft split the trait data half into
`u8 data | u8 level` with `data` carrying reach — the user withdrew it when
`emit_light` became a def-side parameter ("Level is hand authored to define
what color, what intensity, what reach… we then do not need to break this into
u8 data, u8 level"). The split remains AVAILABLE to a future stream if a trait
ever needs per-bind scalar data; nothing in this stream forecloses it.

## F4 — `emit_light` is a PER-LEVEL PARAMETER TABLE on trait defs {#f4}
_2026-08-08 · the user's revision, adopted as the mechanism_

**Chosen.** Any trait def may author `emit_light` entries per level — no
hard-coded trait names anywhere. Each level entry authors the WHOLE tuple:
`color`, `intensity`, `reach`, `fall_off`, `elevation` (how far up the light
sits — a torch emits from its flame, not its base; today's `height` lane),
`radius`, `flicker`, cast/hot flags — with room for future variables
(direction…). The bound LEVEL selects the tuple. The derivation is then pure:
does the object have traits → does a bound trait author emit_light at that
level → it emits that light.

**Why.** Level-count validation already bounds binds against authored tables
(the emotions machinery), so a per-level parameter block slots straight in,
and the corpus stays the single authority on what light means at each level.
**Rejected — a raw color/reach byte in the row** (first draft): couples light
to row layout and to a 256-palette that is not in the repo. **Rejected — a
hard-coded `emit_light` TRAIT NAME**: the user's revision explicitly
generalizes to a parameter any trait can carry.

## F5 — ONE merged-traits accessor; constant derives, runtime rides payload {#f5}
_2026-08-08 · resolved at plan time_

**Chosen.** A single shared/content accessor answers "what traits does this
object carry": the def's CONSTANT assignments (derived, zero storage) merged
with the payload's runtime rows (pawns, exactly today's lane). `mint_sidecars`
stops minting rows for constant binds. All readers — stat_eval, emotions,
affordance Tag checks, inventory capacity, the client's light derivation and
panels — move to the accessor; it is also the ENFORCEMENT point: a write path
targeting a constant assignment refuses loudly ([I4](issues.md#i4)). On a name
collision the CONSTANT assignment wins and the runtime row is ignored
(stated, simplest).

**Why.** The ONE-eval law (needs-moodlets) already forces shared logic; a
second ad-hoc "def traits + payload traits" merge per consumer is exactly the
drift that law exists to prevent.

## F6 — things accept CONSTANT assignments only (v1) {#f6}
_2026-08-08 · resolved at plan time_

**Chosen.** Load validation refuses a non-constant trait bind on a non-pawn
thing. Runtime trait attachment to THINGS is a NAMED SUCCESSOR (it needs a
thing-side gameplay sub-table that does not exist).

**Why.** The user's own constraint: a thing carrying non-constant traits
cannot be saved to cold — and cold is where things live. Refusing at load is
the honest v1; silently accepting-and-losing rows on the next cold save would
be the worst outcome. Pawns keep both flavors (payload storage exists).

## F7 — `visual.light` DELETES; `thing_light()` derives from the traits {#f7}
_2026-08-08 · resolved at plan time_

**Chosen.** Both torches convert to a constant light-carrying trait bind; the
`light =` block and its `LightToml` lane die in the same change (delete, don't
deprecate). `thing_light()` keeps its stride-8 output shape but composes it
from the merged traits' emit_light tuples — the client's `lightFor(kindId)`
and the whole cold-baking path stay untouched ([I3](issues.md#i3)).

**Why.** Two authoring lanes for one phenomenon is how the corpus rots; the
consumer shape staying constant makes the deletion cheap.

## F8 — multiple light-carrying traits ALL attach {#f8}
_2026-08-08 · resolved at plan time_

**Chosen.** Every bound trait whose level authors emit_light contributes a
light, up to the prim's piece budget (≤4 pieces; excess drops loudest-first by
reach, logged). Lights already accumulate by the max-accumulate invariant, so
two sources on one object is a rendering non-event.

**Why.** Any first-one-wins rule invents an authoring order dependence the
corpus never had. **Rejected — refuse multiple**: a burning torch-bearer is a
legitimate future.
