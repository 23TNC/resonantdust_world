# Pathfinding — impathable tiles, impathable trees, and walking around them

**What** (user, 2026-08-07): "improve pathfinding. First we will implement impathable
tiles. I suspect we would likely want to hold pathfinding tables in spacetime to
accomplish this. We will define water tiles as impathable in toml. Once we have per
tile pathability worked out, we will define trees as impathable in toml. This means we
will not be able to travel onto tiles that contain trees. Similarly when we remove
trees we will be able to path through them. I suspect this will be a complex problem.
To assist with debugging… add a variable we can pass through the url to control the
ambient minimum. You can use 0.8 for these tests so you can see beyond the range of
this torch for testing water tiles."

## The stance

- **Pathability is DERIVED, never stored** ([F1](forks.md#f1) — the user's
  spacetime-table suspicion is recorded there and is theirs to veto). The worker
  already mirrors the tile AND thing shards' composed baseline ⊕ overlay for every
  zone (interactions P3/F8, lumberjack I1); the client holds the same composed view to
  render; the npc fetches the corpus. A pathfinding table would be a SECOND copy of
  what the shards already say, and the felled-tree case shows why that drifts: the
  thing overlay's kind-0 suppression already reopens the tile the moment the tree
  falls, for every observer, with no extra write.
- **ONE pathfinder** ([F2](forks.md#f2)): `path_eval` in shared/content beside
  `needs_eval`/`stat_eval`/`emotion_eval` — A* on the 8-way tile grid over a
  cell-probe closure, corpus `pathable` flags consulted through the Bundle. Consumers:
  the worker (authoritative hops), the wasm client (speculation must glide the SAME
  detour or pawns rubber-band — [I1](issues.md#i1)), the npc (trip-time estimates).
- **The chain shape survives**: `MOVE_STEP` keeps its `[obj, dest, serial]` operands
  and supersession law; only the step rule changes — greedy `signum` becomes "the
  first hop of the shared path, recomputed per hop" ([F3](forks.md#f3)). Stateless
  per-hop recompute is what keeps re-issue/supersession safe with no stored route.
- **The corpus owns the flags**: `pathable = false` on the water TILE def, then on the
  tree THING def (kind-level; absence = pathable). A cell is pathable iff its composed
  tile kind is pathable AND no impathable thing occupies it.
- **Refusals are quiet no-ops** ([F5](forks.md#f5)): an impathable or unreachable
  destination logs and no-ops the trip, the same posture as intent-completion
  re-validation. A pawn standing IN an impathable cell may always LEAVE ([F6](forks.md#f6)).
- **The debug knob first**: URL `ambient=<0..1>` overrides the blit's ambient floor
  (hardcoded 0.12 today) so water drills read at 0.8 without torchlight ([F7](forks.md#f7)).

Authoritative docs touched: `docs/ACTIONS.md` §Movement (the pathing law),
`docs/VARIABLES.md` (the `pathable` flag on tile/thing defs).
