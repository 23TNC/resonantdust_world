# Forks — interactions

## F1 — gameplay definitions join the REGISTRY: type "gameplay", numbered like things {#f1}

_2026-08-06, **user**: "Lets assign u32 id's to our needs/trait/interaction/affordance/condition
and other gameplay definitions. We will use the same system as we do for things. We will use u4
type 'gameplay', u12 subtype condition, affordance, interaction, trait etc. u12 kind so…
'gameplay/interaction/drink', this leaves variation… we will use 'default'."_

**Chosen (user)**: ONE id system. Needs, conditions, traits, interactions, affordances — every
gameplay definition authors a taxonomy (`type = "gameplay"`, subtype = the category, kind = the
def, variant `default`) and the SERVER numbers the tuple into `index.definitions`, exactly as
tiles/things do. The explicit-`id = N` law in `content/needs.toml` DIES — registry allocation
provides the same never-renumber/never-reuse guarantee it was reaching for, plus versioning for
free. Variants are a live lane, not a placeholder: later `angry`/`sad` variations of an
interaction, with the AFFORDANCE specifying which variations it offers. **Supersedes** this
plan's first draft (explicit ids for wire-riders) — and pulls needs/conditions through the same
door, which is what forces [I1](issues.md#i1) (payload words must grow to hold u32 ids).

## F2 — the affordance is its OWN def; carriers reference it by name + parameters {#f2}

_2026-08-06._ `[[affordance]]` binds `requires` (traits) to an `interaction` (+ which of its
variants are available, per F1) ONCE; a carrier def references it with ITS values:
`affordances = [{ name = "drink_water", magnitude = 3 }]` on the water tile, magnitude 1 or 5 on
a future waterskin. **Rejected**: inlining the binding per carrier — restates the trait gate
everywhere (the two-copies class, in data).

## F3 — magnitudes are RAW satisfaction units, signed; wire words are BIAS-encoded {#f3}

_2026-08-06, **user**: "Needs will be held as u8, and have -128 bias, so we can essentially hold
an i8. … We will treat the input as i32 so we can handle +/- 3 etc. So we subtract from whatever
we pass it."_

**Chosen (user)**: no unit multiplier — "drink 3" means +3 in the satisfaction domain itself.
The satisfaction lane stays u8 STORED but carries a −128-biased i8 VALUE; event inputs are u32
words carrying bias-encoded i32 values, decoded by subtraction at the consumer. One rule, both
widths: stored = value + bias; effects are signed, so damage-a-need is the same mechanism as
satisfy. **Supersedes** the first draft's `magnitude × unit` — the scale question it solved is
now [F7](#f7)'s domain mapping instead.

## F4 — execution is an EVENT queued through the edge; the worker interprets it {#f4}

_2026-08-06, **user**: "we likely want to queue an 'event' through edge in spacetime to execute
an 'interaction'. Workers then are able to execute the interaction. Therefore the event will
likely be something like u32 'execute interaction' u32 'interaction id' u32 'version' u32 'input
count' vec<u32> input… our event says 'execute interaction' 'gameplay/interaction/drink/default'
3 'pawns/animal/wolf/N' 'gameplay/needs/thirst/default' 3."_

**Chosen (user)**: `EXECUTE_INTERACTION` is a variable-arity event (the CREATE precedent):
`[op, interaction_id:u32, version:u32, input_count:u32, inputs…]`, riding the existing
client→edge-queue→worker path. The WORKER — holding the TOML corpus + the spacetime gameplay
manifest — resolves the interaction def and executes it against the decoded inputs, landing
effects through the existing `SET_NEED`/`GRANT_CONDITION` splices. `version` is explicit in the
event because registry versioning is real: the def executed is the one named, not `max(version)`.
This turn the worker executes as instructed (plus the [F8](#f8) "on"-tile check) — the
AFFORDANCE is consulted by the INITIATOR for availability; deeper authority is a later
refinement ([I4](issues.md#i4)). **Supersedes** the first draft's two-operand relay verb.

## F5 — the interaction declares an INPUT SIGNATURE; effects bind inputs or constants {#f5}

_2026-08-06._ "Our interaction will accept variables" (user). Drink's def declares WHAT it needs
(a target pawn, a need, an amount) and WHAT it does (subtract the amount from the need; grant
`quenched`); the caller supplies the actuals, the carrier's affordance entry having bound the
magnitude. Effects are named declarative fields (`satisfy`, `grant`) whose operands are either
input slots or constants — a new interaction is a TOML edit, and `quenched` stays baked into
drink while need + amount flow in. **Rejected**: a generic effect expression language — that is
the DSL this corpus just retired.

## F6 — traits are per-KIND today, checked through a per-PAWN shaped API {#f6}

_2026-08-06._ All wolves are biological lifeforms: the assignment is `traits = [...]` on the
thing def, like `needs`. The availability check takes A TRAIT SET, resolution building it as
(kind traits) ∪ (payload traits — empty today), so individual-pawn traits later (a payload
opcode, per the human-pawns payload design) move nothing. **Rejected**: kind-lookup hard-coded
at gate sites.

## F7 — the i8 satisfaction domain: full = 127, empty = 0, negative = deficit {#f7}

_2026-08-06, mine — flagged for the user's veto._ F3 fixes the STORAGE (biased i8) but not the
MAP. **Chosen**: full satisfaction = 127, the depletion target = 0, and the negative half is
DEFICIT territory (a need driven below empty by effects or future mechanics); `deplete` stays
TICS for the full→empty traverse (127→0), the rate continuing below zero; band `lo`/`hi` and
magnitudes author in the SAME raw units (thirsty = `lo 13, hi 45` replacing 0.1/0.35 of the old
255 scale). One domain everywhere kills the fraction↔raw conversion class. **Rejected**: keeping
fractional bands over a signed domain (fractions of WHAT range? — every consumer must agree on
an answer the corpus no longer states).

## F8 — this turn's location rule is ON the tile; unit.x/y/z is the recorded generalization {#f8}

_2026-08-06, **user**: "the interaction this turn will specify that we will execute the
interaction 'on' the tile… I suspect we can generalize this to, unit.x/y/z and execute a 'move
to and execute' against unit.x/y/z. The interaction can then hold self.unit.x/y/z for those
for… self, and other values otherwise."_

**Chosen (user)**: the worker's placement check is: the target pawn STANDS ON a tile whose def
carries the affordance. Adjacency, passability, and other placements are LATER refinements; the
documented destination is location generalized to `unit.x/y/z` (an interaction holding
`self.unit.*` or other values) paired with a queued "move to and execute" event so arrival and
execution need no round-trip — declared, NOT built this turn.
