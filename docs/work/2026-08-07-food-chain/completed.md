# Completed — food chain

## 2026-08-07 — P0/P1: the paper + the machinery

- VARIABLES.md gained the corpus laws (0..2 encoding domain; the LEVELED corpus trait
  caps it, HIGHEST-wins across trait max contributions; mint at the effective max),
  the need-check affordance (+ its role as the trigger key), and the death/forage
  interaction schema (`location = "self"`, `spawn = {thing, at}`, `remove = "target"`).
  Verified: `bin/rd docs-check` green.
- Loader: `AffordanceCheck::Stat|Need` (+ `trigger_need()`), `SpawnEffect`, `remove`;
  refusals for a check that is neither form, both forms, or multiple ops; spawn
  validates the thing exists and `at ∈ on|adjacent`. Verified: content lib tests green
  (round-trip + refusal test `the_food_chain_surfaces_round_trip_and_refuse`).
- needs_eval: `need_bounds` tiers need CAPS upward (highest-wins — unlike stat
  intersection); `need_satisfaction` public for the check. Verified: unit test
  `need_caps_tier_upward_by_the_leveled_trait` (bunny cap 1, wolf cap 2).
- stat_eval: `affordance_passes`/`interaction_available` evaluate need checks lazily
  beside stat checks. Verified: `a_need_check_gates_on_the_lazy_value`.
- Worker mint: `mint_sidecars` quantizes the EFFECTIVE max via `need_bounds` (I2).
  Verified: in-binary test `needs_mint_at_the_effective_max` + live SQL — the minted
  bunny row half-encodes (0x8000 = 1 on 0..2), the wolf full (0xFFFF = 2).
- Worker: the NEED-WRITE TRIGGER (F5, the user's law) — every SET_NEED sweeps the
  target's carried interactions for need-check affordances keyed on the written need,
  substitutes the FRESH write into the mirrored rows, queues passers at master+4.
  Verified: one client SET_NEED corpus→0 produced
  `need-write trigger fired (food-chain F5) … interaction=death need=corpus`.
- Pawn shard: the `remove` reducer (entity_state + payload + needs; silent on
  absence). Redeploy sequenced per I1 (wipe accepted, cast re-minted); bindings
  regenerated for edge + st-bindings. Verified: a CLI remove deleted the rows and a
  second call no-opped.

## 2026-08-07 — P2: the corpus

- needs.toml: hunger (0..100, deplete 43200; hungry [10,35) / starving [0,10) with
  uncomfortable/scared emotions), corpus (0..2, no deplete, no bands).
- interactions.toml: stats foraging/plant_eating/meat_eating; the leveled corpus
  trait (`max = [1.0, 2.0]`) + forager/herbivore/carnivore/omnivore; interactions
  death (self, can_die, spawn meat on, remove target), forage (adjacent, spawn
  plant_matter adjacent, 30 tics), eat_plant_matter / eat_meat (adjacent, satisfy
  hunger, destroy carrier, 20 tics); affordances can_forage/can_eat_plants/
  can_eat_meat (stat above 0) + can_die (need corpus ≤ 0).
- things.toml: bunny (walks 1, corpus 1, herbivore), meat, plant_matter; wolves
  corpus 2 + carnivore; humans corpus 1 + forager + omnivore; all four pawn kinds
  carry death; flora carries forage. Verified: golden re-blessed shows exactly the
  authored rows; master seeded 226 defs; worker loads 15 kinds / 7 interactions;
  all six consumers rebuilt; browser wasm cache-busted.
- Placeholder outline (F9): `uOutline` in the MRT bake (8% UV border → black),
  `Primitive.outline` set at all three prim creators (`!tex.name` — the addPrim
  explicit-copy gotcha honored). Verified: capture — every textureless tint square
  (shrub, logs, meat, plant matter) reads outlined; real art untouched.

## 2026-08-07 — P3: the worker executes

- Death: validate can_die → spawn meat at the holder's cell → `remove` → queue
  cleared (I7). Verified live: the bunny died 2 tics after the write — meat (kind 14)
  at (100,74), the mover dropped client-side, a re-fire on the corpse triggered
  NOTHING (idempotent, I3).
- Forage + eats: forage dropped plant_matter at the first empty pathable adjacent
  cell — flora standing; the human's eat walked, satisfied hunger 98.06→100,
  destroyed=true, the cell composed back to 0.

## 2026-08-07 — P4: the bunnies (and the npc learns things)

- Bot thing mirror (F8/I4): `ColdThings` baselines ⊕ `ColdState` TYPE_BIOME_THING
  overrides, kind-0 suppressed, `ZoneClosed` clears; `thing_kind_at` +
  `nearest_thing`. Verified live: no brain ever re-targeted an eaten thing.
- `nearest_thing`'s predicate gained the CELL: brains skip food on impathable ground.
  Found live: the wolf oscillated forever between the worker's F5 refusal (the
  drowned bunny's lake meat) and its wander deadline.
- brains/bunnies.rs: a GROUP brain — 3 minted/adopted at the warren (113,68),
  per-member Minds, wander/drink/eat, warren replenishment (mint guard re-mints
  while minds < count, bounded per run). Verified: soak — roam + drink; a STARVING
  bunny autonomously walked to plant matter and ate 9.37→39.37 destroyed=true.
- wolves gained mind_eat (hunger band → nearest usable eat → walk → adjacent fire).
  Verified: the starved wolf logged `hungry — heading to food`, walked, ate.
- FOUND+FIXED (busy-hold): the wander walked the eater out of range during the
  20-tic eat — `completion NO-OP: the pawn left the carrier's range`. Both brains
  now HOLD STILL for the act's duration (+margin); expiry re-arms the eat latch so
  a no-opped completion retries.
- FOUND+FIXED (I11): a removed pawn's in-flight chain resurrected it from
  entity_state_LOG history (a zombie with no payload, seen live on the wolf). The
  compose now drops pawn targets with no LIVE `entity_state` row.

## 2026-08-07 — P5: the chain, end to end

Every link fired live, in worker/npc logs:

1. **Forage** — the human's forage dropped plant_matter beside the flora at
   (113,73), flora standing (the P3 drill; the spawn scanned to the first empty
   pathable adjacent cell).
2. **Herbivore eats** — the starving bunny walked and ate:
   `eat_plant_matter target="0x30800005" satisfied=Some("hunger") from=9.37
   to=39.37 destroyed=true`.
3. **Death by trigger** — ONE `SET_NEED corpus→0`:
   `need-write trigger fired (food-chain F5) … interaction=death need=corpus` →
   `the holder died — removed (food-chain F5)` → meat at the corpse's cell.
4. **Carnivore eats** — the starved wolf (minted corpus 2 — the leveled trait):
   `hungry — heading to food` → walked to (110,64) →
   `eat_meat target="0x30800007" satisfied=Some("hunger") from=0.0 to=30.0
   destroyed=true` — the busy-hold kept it in range for the 20-tic act.
