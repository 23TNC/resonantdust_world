# The food chain — hunger, bunnies, foraging, meat, and death

**What** (user, 2026-08-07): biological life gains a HUNGER need beside thirst. A BUNNY
pawn (biological life) with an npc module handling GROUPS of bunnies. A need CORPUS on
biological-life pawns, min 0 max 1; a trait CORPUS I on wolves raising the health
need's max to 2 (min 0). A DEATH interaction on biological life — afforded when the
holder's health need ≤ 0 — that spawns MEAT at the holder's location and REMOVES the
holder. MEAT defined as a thing (like logs). A FORAGER trait; a FORAGE interaction on
flora (affordance: has forager) generating PLANT MATTER on an adjacent tile. A
HERBIVORE trait; EAT_PLANT_MATTER on plant matter (affordance: has Herbivore or
Omnivore). Forager on humans. CARNIVORE and OMNIVORE traits; EAT_MEAT on meat
(affordance: has Omnivore or Carnivore). Both eats satisfy hunger. Wolves get
carnivore, humans omnivore. (A coming turn adds wolves attacking bunnies.)

## The stance

- **The diet/ability gates are the lumberjack pattern** ([F3](forks.md#f3)): traits
  contribute stats, affordances are predicates on them. `forager → foraging +1`,
  `herbivore/omnivore → plant_eating +1`, `carnivore/omnivore → meat_eating +1`;
  `can_forage`/`can_eat_plants`/`can_eat_meat` gate on `> 0`. "Has X or Y" is a sum —
  no new machinery.
- **Corpus is the health need**, encoding domain 0..2 with TIERED caps
  ([F2](forks.md#f2)): need MAX modifiers take the HIGHEST authored value (an
  unexercised lane today — no corpus content uses need min/max modifiers), so
  `biological_lifeform` caps corpus at 1 and `corpus_i` raises wolves to 2. Needs
  mint at the EFFECTIVE max (bunnies 1, wolves 2), `deplete = 0` — health only moves
  by events.
- **Death's affordance is a NEED predicate** ([F4](forks.md#f4)): affordance checks
  gain a `need` variant beside `stat` (`{ need = "corpus", lte = 0 }`) — the ONE
  shared eval gates it for menu, npc, and worker alike.
- **Death fires from the NEED-WRITE TRIGGER** ([F5](forks.md#f5) — the user's design,
  given mid-plan): every worker need write SWEEPS the target's carried interactions
  for affordances holding a NEED check on the modified need and queues the ones that
  pass — the mutation is the trigger, no polling, no brains. The effect writes MEAT
  into the holder's cell (the yield lane) and REMOVES the pawn via a NEW pawn-shard
  `remove` reducer (the module has none; the redeploy wipes live pawns — I1).
- **Forage spawns beside, eats consume** ([F6](forks.md#f6)/[F7](forks.md#f7)):
  forage leaves the flora standing and places plant_matter on the first EMPTY
  PATHABLE adjacent cell (deterministic scan; the pathfinding probe decides "empty");
  eat_plant_matter/eat_meat destroy their carrier and satisfy hunger.
- **The npc learns THINGS** ([F8](forks.md#f8)): the Bot already receives ColdThings
  — it now stores them (the mirror pathfinding F8 deferred), so bunny groups
  (`brains/bunnies.rs`, NPC_BUNNIES count) eat plant matter and hungry wolves eat
  meat. Placeholder art for bunny/meat/plant_matter ([F9](forks.md#f9)).

Authoritative docs touched: VARIABLES.md (needs, the cap-tier law, the need-check
affordance, new defs), ACTIONS.md (the death/spawn effects).
