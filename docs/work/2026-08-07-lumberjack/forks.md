# Forks — lumberjack (plan-time decisions; each is mine unless the user vetoes)

## F1 — the intent queue lives in the pawn shard, one row per pawn {#f1}

**Chosen**: a pawn-shard `intents` table row per pawn — `entity_reference`,
`macro_position_reference` (zone-slaved like `needs`), `intents: Vec<u32>` (the queued
program: packed `EXECUTE_INTERACTION`-shaped entries), `head_started_tic: u32`. Slaved to
the state claim like `payload` (never claimed independently); fanned whole-row on change
via a new wire frame.
**Rejected**: worker in-memory (a worker restart forgets orders — against the self-heal
posture; invisible to any future UI); one row per slot (no atomic replace, wider wire);
payload opcodes (payload is mint/identity state, churning it per order is wrong-shaped).

## F2 — "adjacent" = Chebyshev ≤ 1 INCLUSIVE of the carrier cell {#f2}

"On or beside." Inclusive keeps every live arc green — the wolf npc walks ONTO water and
drinks there; the redux drills drank standing on the pond tile. Exclusive adjacency would
break the npc brain and all standing drills for zero design gain (there is no occupancy
model yet to make standing-on impossible). Cut-down from atop the tree cell is accepted
dev physics for the same reason.

## F3 — supersession: a fresh order REPLACES the whole queue; cap 5 {#f3}

A new player/npc-issued interaction clears the pawn's queue and installs its own
composition — the MOVE_STEP "ONE chain per pawn, re-issue safe" law lifted one level (the
queue's head move_to still supersedes the movement chain exactly as today). Preemption =
issue a new order. The 5-cap applies to a single composition; exceeding it rejects the
whole order with a logged reason ([I8](issues.md#i8)). Rejected: append semantics
(shift-click queuing is a later UI conversation; append + replace both live in one law
badly).

## F4 — humans author `lumberjack` level 1 in things.toml {#f4}

Both human kinds gain `{ name = "lumberjack", level = 1 }` beside `walks` — every human
can chop (dev posture, mirrors how walks/biological_lifeform bind today); wolves do not
(the negative case is free: the wolf's menu on a tree offers no Cut Down, and that refusal
is the affordance gate working). Rejected: a `/spawn … trait <name>` arg (a second dev
door to build and drill for no design content this stream needs).

## F5 — `destroy = "carrier"`; the logs successor is `yields`, not built {#f5}

The destroy effect names the CARRIER — the validated offerer whose def carries the
interaction binding — so the worker already holds the (cold_row, cell) to clear:
`PROMOTE SET <cold_row> TYPE_BIOME_THING <cell> 0 0` (kind 0 = remove; the proven
build-walls SET path). The next stream adds `yields = "logs"` beside `destroy` and the
composer emits a placing SET instead of a clearing one — additive, no reshape. Rejected:
`this.destroy` object-path syntax (nothing else in the corpus dots through an object;
`destroy = "carrier"` matches the existing operand vocabulary).

## F6 — cut_down carriers: tree, shrub, cactus {#f6}

The "tree-like" scatter set. Excluded: rock (not tree-like), reed/flora (groundcover, and
felling grass reads wrong), torches (placed lights — removal is a different conversation).
The user can widen the list with one TOML line each.

## F7 — names and numbers: stat `logging`, affordance `can_fell_trees`, 30 tics {#f7}

`[[stat]] logging` (min 0, max 10, winner "min"), `[[trait]] lumberjack`
`stats = [{ stat = "logging", add = [1] }]`, `[[affordance]] can_fell_trees`
`check = { stat = "logging", above = 0.0 }`, `[[interaction]] cut_down`
`menu_text = "Cut Down"`, `duration = 30` (≈5 s at 6 Hz — long enough to watch the tics
elapse in a drill, short enough to drill repeatedly).
