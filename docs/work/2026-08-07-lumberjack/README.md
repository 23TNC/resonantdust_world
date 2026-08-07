# lumberjack — timed work on the world: the trait, adjacency, the intent queue, felled trees

**What** (user, 2026-08-07): add a `lumberjack` trait and a cut-down-trees interaction whose
affordance is *pawns with the lumberjack trait adjacent to the tree*; update `drink` to also
require adjacency; implement a per-pawn **intent queue** (max 5) so a move_to intent can be
queued ahead of a positionally-gated interaction, so queued actions can be preempted, and so
the queue can track **tics spent** on the head action — the tool that lets interactions cost
tics. `cut_down` removes the tree after its authored tic cost elapses, with the removal
specified in the interaction's TOML (`destroy …`); the same interaction applies to other
tree-like things. **Named successor (next stream, not built here): place a thing — logs —
where the tree stood.**

## Why now

The corpus reserved exactly these seams at stat-model/input-rework time:
`duration = 0` sits in `interactions.toml` marked "RESERVED (I9): interactions are
instantaneous; a timed state must negotiate with MOVE_STEP chain supersession and gets its
own stream", and the `location = "on"` comment records "unit locations + a queued *move to
and execute*" as a later stream. This is that stream — the intent queue is the negotiation
mechanism I9 asked for.

## Design stance

- **Everything through the four-family model** (stat-model): `lumberjack` is a leveled
  TRAIT contributing to a new `logging` STAT; the gate is an AFFORDANCE
  (`can_fell_trees`, `logging > 0`) on the INTERACTION. No special-case code for who may
  chop — a golem with an authored logging stat could chop; a wolf cannot.
- **Adjacency is a location rule, not a predicate**: a third `location` value,
  `"adjacent"` = Chebyshev ≤ 1 **inclusive of the carrier cell** ([F2](forks.md#f2)) —
  "on or beside". `drink` moves from `"on"` to `"adjacent"`; every existing arc (the wolf
  standing on water) stays green by construction.
- **The intent queue is shard state, not worker memory** ([F1](forks.md#f1)): one row per
  pawn in the pawn shard, slaved to the state claim like `payload`, fanned on change. A
  worker restart must not forget orders (sim-self-heal ethos), and a fanned queue is what
  a future queue UI reads.
- **The queue is how positional interactions compose**: the menu offers a positional
  interaction even from afar; the worker (ONE composer — the same place the affordance
  gate lives, so npc and menu cannot drift) composes `[move_to(adjacent), act]` into the
  queue when the pawn is out of place. A fresh order REPLACES the queue
  ([F3](forks.md#f3)) — the MOVE_STEP one-chain law lifted one level.
- **Tic costs live on the head intent**: the head stamps `started_tic` when it begins;
  the worker pops it when `elapsed ≥ duration` and only then queues the effect writes.
  `duration = 0` intents (drink, move_to seeds) execute-and-pop in one pass — today's
  behavior is the degenerate case.
- **Removal is the existing cold-overlay write**: `destroy = "carrier"` resolves the
  validated offerer cell and emits `PROMOTE SET <cold_row> TYPE_BIOME_THING <cell> 0 0` —
  the same SET the build-walls stream proved, kind 0 = remove ([F5](forks.md#f5)). The
  logs successor becomes an additive `yields = "<thing>"` beside it; the TOML shape is
  chosen so that lands without reshaping.

## What exists (audited 2026-08-07)

- Worker EXECUTE_INTERACTION arm: resolves corpus signature, location rules `"on"` /
  `"target"` (tile-carrier only), affordance predicate gate shared with the npc, queues
  typed writes at `master+4`. `server/worker/src/main.rs` ~710–960.
- Thing shard: sparse baseline ⊕ overlay, `kind_reference == 0` removes; SET routes by
  `TYPE_BIOME_THING`. No worker-side thing-carrier validation yet ([I1](issues.md#i1)).
- wasm menu filter: `tileMenuOptions`/`thingMenuOptions`, suppresses `"on"` interactions
  unless standing on (`shared/wasm/src/lib.rs` ~710–731).
- npc wolves: `usable_drink` + `nearest_tile` walk-onto-water-then-drink.
- Humans/wolves author traits in `things.toml`; mint sidecars land at CREATE.
