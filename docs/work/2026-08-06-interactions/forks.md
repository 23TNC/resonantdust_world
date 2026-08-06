# Forks — interactions

## F1 — trait/interaction/affordance ids are EXPLICIT, not registry-numbered {#f1}

_2026-08-06._ The corpus now holds TWO id regimes: tiles/things author a taxonomy and the SERVER
numbers them (definition-registry), while needs/conditions author `id = N` because their ids are
STORED data (payload words, wire operands).

**Chosen**: the three new categories follow the needs regime — explicit ids, loader refuses
duplicate/zero/missing, never renumber, never reuse. The affordance id rides the
`EXECUTE_INTERACTION` wire verb, and a future per-pawn TRAIT payload opcode will store trait ids;
both make the id data the corpus must pin. **Rejected**: registry numbering — the registry
allocates ids for PLACEABLE world objects (taxonomy tuples with versions); traits and
interactions are rule vocabulary, not placements, and forcing them through `(type, kind, subType,
variant)` would be a category error. The schema must state this split rule plainly
([I1](issues.md#i1)).

## F2 — the affordance is its OWN def; carriers reference it by name + parameters {#f2}

_2026-08-06._ Where does "biological_lifeform may drink" live?

**Chosen**: `[[affordance]]` is a standalone def binding `requires` (trait list) to an
`interaction`; a carrier (tile or thing def) authors `affordances = [{ name, magnitude }]` — the
reference plus ITS parameters. This is the user's own three-entity vocabulary, and it means the
trait gate is authored ONCE: the waterskin at magnitude 1 and the pond at magnitude 3 share one
`drink_water` binding. **Rejected**: inlining the whole binding on each carrier — every carrier
would restate the trait gate, and a gate tune becomes an every-carrier edit (the two-copies
class, in data).

## F3 — magnitude semantics: effect = magnitude × the interaction's `unit` {#f3}

_2026-08-06._ Needs are 0..1 satisfactions in an 8-bit lane (0..=255). "drink 3" must mean
something exact.

**Chosen**: the INTERACTION authors `unit` — satisfaction per magnitude point (drink authors
`unit = 0.1`, so drink 3 = +0.3 satisfaction = +77 in the u8 lane, clamped at 255). Carriers
stay small-integer authorable exactly as the user spoke them ("drink 1 or drink 5"), and
rebalancing every drink source is ONE edit to the unit. **Rejected**: raw u8 magnitudes
(unauthorable — nobody writes "drink 77"); a hard-coded global scale (untunable per
interaction — eating and drinking will not want the same grain).

## F4 — the WORKER resolves interactions; the npc only requests {#f4}

_2026-08-06._ Today `SET_NEED` is npc-composed and relay-trusted. The drink effect could be the
same: npc computes the new satisfaction and sends `SET_NEED` itself, zero new server logic.

**Chosen**: server-authoritative — `EXECUTE_INTERACTION(pawn, affordance)` is resolved by the
WORKER: trait gate, carrier-def check on the pawn's tile ∪ 4-neighborhood, current satisfaction
via the shared `needs_eval` at now-tic, then the existing `SET_NEED` + `GRANT_CONDITION` splices.
**Rejected**: npc-side composition — it duplicates the effect computation outside the ONE eval
posture, and it means "drink anywhere, drink anything" is a client's choice; the interaction
system is exactly where authority should START growing ([I4](issues.md#i4) records that the raw
`SET_NEED` door stays open this stream — closing it is an ownership/auth stream).

## F5 — an interaction carries an EFFECT LIST, not a hard-coded satisfy field {#f5}

_2026-08-06._ Drink does two things already (satisfy thirst, grant quenched), and conditions F6
established that effects are an open set.

**Chosen**: the interaction schema is `satisfy = { need, ... }` (magnitude-scaled) plus
`grant = [condition, ...]` (timed grants land through `GRANT_CONDITION`, expiry from the
condition's own `duration`) — named effect fields with room beside them, the same posture as the
condition's `mood`. A future interaction adds a field (damage, a stat), not a schema rewrite.
**Rejected**: a generic opcode/expression effect language — that is the DSL this corpus just
retired; effects stay declarative fields until a measured need says otherwise.

## F6 — traits are per-KIND today, checked through a per-PAWN shaped API {#f6}

_2026-08-06._ All wolves are biological lifeforms, so the corpus assignment is on the kind
(`traits = [...]` on the thing def), like `needs`.

**Chosen**: the gate function takes A TRAIT SET, and resolution builds it as (kind traits) ∪
(payload traits — empty today). When individual pawns grow traits (a payload `TRAIT` opcode is
the obvious lane, per the human-pawns payload design), the gate does not move. **Rejected**:
kind-lookup hard-coded at gate sites — it would make per-pawn traits a find-every-callsite
refactor later, the exact future-intent trim the conventions forbid.
