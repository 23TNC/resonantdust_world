# Traits, interactions, affordances — the pawn learns to DO

_Opened 2026-08-06 (user): "we are going to also define traits, interactions and affordances in
toml. An interaction is something a pawn can do, and defines how the pawn can do it and what it
does. An affordance defines what interactions are available to the pawn. In this case, we would
like to define the capability drink water, and assign it to the water tile objects. We will define
a trait Biological Lifeform, and assign it to our wolf pawns. We will define an affordance, pawns
with Biological Lifeform may execute the drink interaction. The drink interaction will satisfy the
thirst need. Because we want our interaction to be generic, our interaction will accept variables.
So, our water tile might have drink 3 to satisfy 3 thirst need, and another object that carries
drink might have drink 1 or drink 5 to satisfy different amounts of thirst."_

This is the **declared successor** of [2026-08-03-needs-moodlets](../2026-08-03-needs-moodlets/README.md)
("the drink action is the successor stream") and the reason the TIMED condition lane exists:
`quenched` (id 3, duration 3600) has been sitting in `content/needs.toml` since that stream,
exercised only by the `NPC_GRANT` drill. This stream gives it its real producer.

## The model

Three new **explicit-id** corpus categories ([F1](forks.md#f1) — they ride the wire/payload like
needs and conditions, so the registry does NOT number them):

- **`[[trait]]`** — a capability class a pawn HAS. First: `biological_lifeform`. Assigned per
  KIND in the corpus (`traits = [...]` on the wolf's thing def), checked through an API shaped
  for per-PAWN traits later ([F6](forks.md#f6)).
- **`[[interaction]]`** — something a pawn can DO: how (gating, later duration) and what it does
  (an effect list — `satisfy` a need, `grant` a condition; [F5](forks.md#f5)). First: `drink`,
  which satisfies `thirst` by `magnitude × unit` ([F3](forks.md#f3)) and grants `quenched`.
  The interaction is GENERIC — the carrier supplies the magnitude.
- **`[[affordance]]`** — the binding that says WHO may do WHAT: `requires` (traits) +
  `interaction`. First: `drink_water` = biological_lifeform may drink. CARRIERS (tile or thing
  defs) reference it by name with their parameters: the water tile authors
  `affordances = [{ name = "drink_water", magnitude = 3 }]`; a future waterskin authors the same
  affordance at magnitude 1 or 5 ([F2](forks.md#f2)).

**Execution is server-authoritative** ([F4](forks.md#f4)): a new `EXECUTE_INTERACTION` verb
carries `(pawn, affordance)`; the WORKER resolves it — trait gate, a carrier def on/adjacent to
the pawn's tile, current satisfaction via the ONE shared `needs_eval`, then composes the existing
`SET_NEED` + `GRANT_CONDITION` splices. The npc only REQUESTS. Nothing new touches the pawn shard
schema — the effect lands through the verbs that already exist.

## Design stance

- The corpus stays DATA: drink is not code anywhere; a new interaction is a TOML edit.
- One eval, still: satisfaction math goes through `needs_eval` on the worker exactly as it does
  in npc and the client. No second computation of "current satisfaction" may exist.
- Wire discipline as in [conditions](../2026-08-04-conditions/README.md): new values append
  (verb 12, new categories), nothing existing moves; the golden fixture re-blesses DELIBERATELY
  with the diff reviewed as added-only ([I8](issues.md#i8)).
- Instantaneous interactions ONLY ([I9](issues.md#i9)): the schema reserves `duration` but this
  stream does not build a timed-action state machine — that collides with MOVE_STEP chain
  supersession and deserves its own stream.
- Straight-line seeking is accepted ([I2](issues.md#i2)): pathfinding is a declared later stream
  (tree-occupancy builds its query); a wolf that can't reach water records the failure, it does
  not block this stream.

## Exit

A drill-scaled wolf lives the full arc UNPROMPTED: thirst drains → Thirsty card → the brain finds
the nearest water, walks there, drinks → satisfaction jumps by the corpus-computed delta →
Thirsty clears, Quenched card shows on the panel → drains again. Captures + logs in
`completed.md`; **the user's eyes close the stream**.
