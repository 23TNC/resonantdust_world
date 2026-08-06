# Traits, interactions, affordances — the pawn learns to DO

_Opened 2026-08-06 (user): "we are going to also define traits, interactions and affordances in
toml. An interaction is something a pawn can do, and defines how the pawn can do it and what it
does. An affordance defines what interactions are available to the pawn. In this case, we would
like to define the capability drink water, and assign it to the water tile objects. We will define
a trait Biological Lifeform, and assign it to our wolf pawns. We will define an affordance, pawns
with Biological Lifeform may execute the drink interaction. The drink interaction will satisfy the
thirst need. Because we want our interaction to be generic, our interaction will accept variables.
So, our water tile might have drink 3 to satisfy 3 thirst need, and another object that carries
drink might have drink 1 or drink 5 to satisfy different amounts of thirst." Revised the same day
by the user's seven answers to the planning review — recorded verbatim in
[`forks.md`](forks.md)._

This is the **declared successor** of [2026-08-03-needs-moodlets](../2026-08-03-needs-moodlets/README.md)
("the drink action is the successor stream") and the reason the TIMED condition lane exists:
`quenched` (timed, 3600 tics) has sat in `content/needs.toml` exercised only by the `NPC_GRANT`
drill. This stream gives it its real producer.

## The model

**One id system** ([F1](forks.md#f1), user): gameplay definitions — needs, conditions, traits,
interactions, affordances — author a TAXONOMY (`type = "gameplay"`, u12 subtype = the category,
u12 kind, u4 variant `default`) and the server numbers the tuple into `index.definitions`,
exactly as things do. `content/needs.toml`'s explicit `id = N` law dies; the registry provides
the same never-renumber guarantee plus versioning. Variation is a live lane: later `angry`/`sad`
interaction variants, with the affordance specifying which are available.

- **`[[trait]]`** — a capability class a pawn HAS: `gameplay/trait/biological_lifeform`.
  Per KIND now (`traits = [...]` on the wolf's thing def), checked through a per-PAWN shaped API
  ([F6](forks.md#f6)).
- **`[[interaction]]`** — something a pawn can DO. It declares an INPUT SIGNATURE and its
  effects bind inputs or constants ([F5](forks.md#f5)): `gameplay/interaction/drink` takes
  (pawn, need, amount), satisfies the need by the SIGNED amount, and grants `quenched`.
- **`[[affordance]]`** — who may do what: `gameplay/affordance/drink_water` = biological
  lifeforms may drink, offering the `default` variant. Carriers bind THEIR parameters
  ([F2](forks.md#f2)): the water tile authors `affordances = [{ name = "drink_water",
  magnitude = 3 }]`; a future waterskin authors 1 or 5.

**Values are float32** ([F3](forks.md#f3), user): satisfaction and magnitudes are f32; an event
input word is the float's bit pattern in its u32 lane. Each need authors its OWN domain — min,
max, sign treatment ([F7](forks.md#f7), user) — with thirst provisionally `0..100` (a percent
scale), so "drink 3" = +3.0 of a 100-full thirst; decimals make percentages and per-tic
gain/loss rates first-class, and signed effects mean damage-a-need is the same mechanism as
satisfy.

**Execution is a queued EVENT** ([F4](forks.md#f4), user):
`EXECUTE_INTERACTION [interaction_id, version, input_count, inputs…]` rides the existing
edge-queue path; the WORKER — corpus + gameplay manifest in hand — resolves the def, decodes
the inputs, checks the pawn stands ON a carrier tile ([F8](forks.md#f8)), computes current
satisfaction through the ONE shared `needs_eval`, and lands the effect through the existing
`SET_NEED` + `GRANT_CONDITION` splices. The npc consults the affordance for AVAILABILITY and
issues the event on arrival; deeper authority is a later refinement ([I4](issues.md#i4)).

## Design stance

- The corpus stays DATA: drink is not code anywhere; a new interaction is a TOML edit.
- One eval, still: satisfaction math goes through `needs_eval` on the worker exactly as in npc
  and the client. No second computation of "current satisfaction" may exist.
- The payload consequence is faced, not dodged ([I1](issues.md#i1)): u32 registry ids outgrow
  the 8-bit payload id lanes, so the NEED/CONDITION entries restructure (def-id word + f32
  satisfaction word) and dev-world wolves re-mint. The f32 domain migration lands everywhere in
  one phase ([I2](issues.md#i2)).
- Instantaneous interactions only ([F8](forks.md#f8)/[I9 of the schema](forks.md#f5)): the npc
  moves the wolf, then issues the event on arrival. The recorded destination — a queued
  "move to and execute" against `unit.x/y/z` so sequenced actions dodge the arrival round-trip —
  is declared by the user for a LATER turn, not built here.
- "On" the tile is the whole placement rule this turn ([F8](forks.md#f8)); passability,
  adjacency, and other locations are later refinements of a working system.

## Exit

A drill-scaled wolf lives the full arc UNPROMPTED: thirst drains → Thirsty card → the brain
moves it to the nearest water, issues `EXECUTE_INTERACTION` on arrival → satisfaction jumps by
exactly +3 → Thirsty clears, Quenched card shows → drains again. Captures + logs in
`completed.md`; **the user's eyes close the stream**.
