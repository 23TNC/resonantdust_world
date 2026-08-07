# Forks — lumberjack (plan-time decisions; each is mine unless the user vetoes)

## F1 — the intent queue is EPHEMERAL worker state (USER, 2026-08-07) {#f1}

**User overruled the plan's shard-row draft**: "The pawn queue doesn't have to be
authoritative… the currently running event is held in our tables as whatever we're
actively working on, and our intent queue is ephemeral running alongside it so… whatever
happens to that queue… happens."

**Chosen**: a per-pawn in-memory pending list at the worker (cap 5, maybe a queue tic).
When the worker completes an event it queues the NEXT one automatically, removing it from
the pawn's list. The DURABLE half is the standing event machinery: the in-flight work is
a queued event in the event shard (exactly like a MOVE_STEP chain hop), so a worker
bounce loses only the pending tail — accepted. **The safety is execution-time
re-validation**: every intent re-checks its affordances AND location at fire — a drink
whose move was cancelled "figures out it's out of range and resolves to a no-op".
**Rejected** (the earlier draft): a fanned pawn-shard `intents` row — authority the
design doesn't need; the no-op law covers loss, and the running event is already in the
tables.

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
