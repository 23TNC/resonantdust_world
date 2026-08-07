# Forks — food chain (plan-time decisions; each is mine unless the user vetoes)

## F1 — needs stay authored PER KIND; "biological life has hunger" = the four kinds {#f1}

The mint reads `needs = [...]` off the thing def (worker `mint_sidecars`). Hunger (and
corpus) are AUTHORED onto wolf/human_male/human_female/bunny — the kinds that carry
`biological_lifeform` — rather than derived from the trait. Trait-derived needs would
move minting into trait resolution for zero present gain.
**Rejected**: needs-from-traits (a mint rework this stream doesn't need; recorded as
the successor if a fifth biological kind ever forgets its hunger line).

## F2 — corpus: encoding domain 0..2, TIERED caps, mint at effective max {#f2}

The stored value quantizes on the AUTHORED min..max (stat-model F4), so the domain
must hold the LARGEST reachable health — `min = 0, max = 2`. The user's "max 1 unless
Corpus I" becomes cap TIERS: `biological_lifeform` authors a corpus `max = [1]` need
modifier, `corpus_i` authors `max = [2]`, and need MAX modifiers combine by **HIGHEST
WINS** — a lane NOTHING authors today (quenched uses `rate` only), so the semantic is
free to define, and "caps tier upward" is what tiers mean. Stat ranges keep their
intersection law untouched. Minting quantizes the EFFECTIVE max (traits are minted
first, so it is computable): bunnies/humans mint 1, wolves 2. `deplete = 0` — health
never decays; only events (damage, healing, the coming attacks) move it.
**Rejected**: authoring 0..1 and letting corpus_i "raise" past the encoding (cannot
store 2); per-pawn quantize domains (every observer must re-derive the domain before
reading any row — a decode landmine).

## F3 — diet/ability gates = the lumberjack stat pattern {#f3}

`forager → foraging +1`; `herbivore → plant_eating +1`; `omnivore → plant_eating +1,
meat_eating +1`; `carnivore → meat_eating +1`. Affordances `can_forage` /
`can_eat_plants` / `can_eat_meat` gate `> 0`. "Has Herbivore OR Omnivore" is the SUM
being positive — the machinery's native disjunction.
**Rejected**: a has-trait affordance primitive (a second gate kind when stats already
express it; the lumberjack precedent holds).

## F4 — affordance checks gain a NEED variant {#f4}

`[[affordance]] check = { need = "corpus", lte = 0 }` beside the stat form. Death's
gate ("health ≤ 0") is a question about a NEED VALUE, and the lazy needs eval already
answers it for every observer — the check evaluates `satisfaction_at` at now. Cmp
vocabulary reuses the biome/stat ops (gte/gt/lt/lte).
**Rejected**: a derived `dead` condition + stat contribution + stat gate (three hops
to say one thing); a hardcoded death rule in the worker (content owns gates).

## F5 — death fires from the NEED-WRITE TRIGGER; spawn meat + REMOVE {#f5}

**The USER's design (2026-08-07, mid-plan): "when a need is modified, is when that
affordance is fired. So… interaction executes on thing, that modifies a need, that
sweeps thing for need trigger affordances, and executes if present. So… when a pawn
looses corpus, it will check against the affordance for the interaction death as it
is tied to the pawns corpus need… in the same turn when corpus reaches 0, death fires
as part of the same work the worker carries out."**

So: wherever the WORKER writes a need (the satisfy effect, SET_NEED, the coming
damage effects), it then SWEEPS the target's carried interactions for affordances
holding a NEED check on the MODIFIED need (F4's variant is the trigger key),
evaluates them, and QUEUES the triggered `EXECUTE_INTERACTION` for each that passes —
no brain involvement, no polling: the MUTATION is the trigger, which is the lazy
philosophy applied to causation. The triggered order rides the same event door as
everything (audit + the fire-time re-validation), landing at `master + 4` — the
barrier's floor. The re-validation makes the 4-tic window CORRECT rather than racy: a
heal inside it saves the pawn (death no-ops); a still-dead pawn dies. A triggered
execution that itself writes needs sweeps again (eat → hunger) — cascades terminate
because each hop re-validates and death ends the chain.

Death's EFFECT: (1) an overlay `SET` writes MEAT's kind_reference into the holder's
floor cell (the logs-drop yield lane); (2) a NEW pawn-shard `remove` reducer deletes
the entity's rows (state, payload, needs) — StateGone fans, clients drop the mover,
the npc un-adopts; the worker clears the ephemeral intent queue (I7). Idempotent:
a second fire finds no pawn and no-ops whole (I3).
**Rejected (superseded by the user)**: brains firing death on the crossing — the
worker owns causation now; brains only ever CHOOSE, never account.

## F6 — forage leaves the flora and spawns plant_matter BESIDE it {#f6}

The user: "generate plant matter on an adjacent tile" — the flora survives (no
destroy). The spawn cell = the first EMPTY, PATHABLE cell scanning the carrier's 3×3
in fixed (dy, dx) order (deterministic; the movement probe answers pathable, the
composed thing view answers empty). All eight full → the forage completes with a
logged no-yield (never an overwrite). `spawn = { thing = "plant_matter", at =
"adjacent" }` on the interaction; duration + queue visuals like cut_down.
**Rejected**: spawning ON the flora's cell (occupied); replacing the flora (the user
kept it distinct from felling).

## F7 — eat_plant_matter / eat_meat CONSUME their carrier {#f7}

Eating destroys the carrier thing (the lumberjack destroy lane, no yield) and
satisfies hunger by the binding's magnitude (the drink pattern — carriers own
magnitudes). Adjacency rule `adjacent`, timed, cancelable, queue-visualed.
**Rejected**: non-consuming food (infinite meat breaks the coming ecology).

## F8 — the Bot stores THINGS; bunnies are a GROUP brain {#f8}

The Bot already receives ColdThings/thing overlays and stored only tiles — it gains
the thing mirror (composed baseline ⊕ overlay, kind-0 suppressed) + `nearest_thing`.
`brains/bunnies.rs` adopts/mints `NPC_BUNNIES` (default 3) bunnies around a home:
wander + drink (the wolves pattern) + eat plant_matter when hungry; the wolves brain
gains eat-meat-when-hungry through the same scan. This closes pathfinding F8's
recorded successor for the npc.
**Rejected**: one-brain-per-bunny processes (the user asked for a GROUP handler);
menu-only eating (npc pawns must feed themselves).

## F9 — bunny, meat, and plant_matter ship as placeholder tint squares {#f9}

The bunny = a single flat-tint PART (small scale, warm gray); meat = a red-brown
square; plant_matter = a fresh-green square — the shrub/logs placeholder convention.
Real art is the sprite pipeline's business, later.
