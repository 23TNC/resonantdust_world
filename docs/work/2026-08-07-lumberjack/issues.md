# Issues — lumberjack (anticipated inventory)

## I1 — the worker's carrier validation is tile-only {#i1}

The EXECUTE_INTERACTION arm resolves the carrier from the TILE shard (`entity_state ⊕
overlay`) and checks `tile_interactions`. cut_down's carrier is a THING — the worker must
resolve the thing at the cell from the thing shard ⊕ overlay and check the thing def's
bindings, **identically to `thingMenuOptions`**, or the menu offers what the worker
refuses (the redux-I2 drift class, again). One resolution helper shared in spirit: the
wasm filter and the worker read the same corpus tables.

## I2 — a timed head intent vs MOVE_STEP supersession {#i2}

The corpus I9 reservation, now due: what happens when a fresh order lands mid-chop? Law
([F3](forks.md#f3)): the queue is REPLACED — the chop is cancelled, no partial credit, no
refund (nothing was spent but time). The elapsed-tics counter dies with the popped head;
the tree is untouched until the full duration elapses in one unbroken tenure. State the
law in ACTIONS.md so the next timed interaction doesn't re-litigate it.

## I3 — tree removal must dirty the COLD caches {#i3}

A felled tree is a cold prim with a baked silhouette shadow. The overlay SET (kind 0)
reaches the client as a cold-cell change — the square rebake must also drop the caster
from the in-family shadow buckets ([shadows-on-prims] rule: read from caster buckets,
never recompute) and re-bake the affected squares, or the tree's shadow outlives the
tree. Build-walls proved the add path; this is the first REMOVE drill.

## I4 — the intents fan needs a wire frame + client mirror {#i4}

Like `Need`: a new edge frame for the intents row, a client `Event::PawnIntents`, and a
mirror the pie-menu/debug can read. Headless drills inherit redux I10's rules (hidden-tab
rAF freeze; MessageChannel yields, never busy-waits).

## I5 — golden + registry churn {#i5}

New stat/trait/affordance/interaction rows, `drink`'s location change, and three thing
defs gaining `interactions` all move the golden corpus tables. Same re-bless discipline
(`BLESS_GOLDEN=1`); dev worlds re-mint nothing (traits mint at CREATE — existing humans
LACK lumberjack until re-minted; state that in the drill, don't be surprised by it).

## I6 — npc parity with adjacency and the queue {#i6}

Inclusive adjacency (F2) keeps `usable_drink` + walk-onto-water green unchanged. But the
npc still composes its own move-then-drink two-step; once the queue lands, the npc COULD
issue one queued order instead. Not this stream's work — record the simplification as a
successor so the brain doesn't fork from the menu path forever.

## I7 — the 6th intent and other rejections are silent to the user {#i7}

Queue-cap and validation rejections log-and-drop at the worker (the I6 interactions law).
The menu has no feedback channel for a refused order. Accepted this stream; the queue row
being fanned means a future UI can at least SHOW the queue it holds.

## I8 — existing pawns predate the lumberjack trait {#i8}

Trait sidecars mint at CREATE. The standing humans (0x30800003–06) carry no `logging`
contributor, so their menus on trees will not offer Cut Down — correct behavior, but the
drill must mint a FRESH human after the corpus lands (the redux worker-bundle-at-startup
gotcha applies: restart the worker before minting).

## I9 — carried successor: logs where the tree stood {#i9}

The user's named next step. The `yields = "<thing>"` TOML shape is chosen (F5) but NOT
built; nothing in this stream may foreclose it — the destroy composer must keep the
carrier's (cold_row, cell) addressing in one place so yields becomes one more emitted SET.
