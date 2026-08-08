# Completed — torch perf

## 2026-08-08 — P2/P3: the measurement

**The reset** — wolves + bunnies stopped, pawn module wiped (0 rows), master
reseeded 236 defs (debug_torch = 805372160), seed guard quiet, pacing 5.97 Hz.
Camera: `focus=100,50&zoom=1&ambient=0.35`, tab foreground. The four placed
world torches (1 warm + 3 blue, cold lights) ride every row as a constant.

**The table** — moving debug_torch pawns (reach-16 warm lights, walks 2), a
≥2-min wandering soak per row, `__framecost().ms` sampled 3-4× and
`droppedLights` (the records' per-tile 8-slot EVICTION counter) sampled over
12 s with no bench calls in between:

| movers | frame ms (spread) | slot evictions/s | worker | notes |
|---|---|---|---|---|
| 8  | 2.60 – 2.71 | ~170 | in step with master | |
| 16 | 2.97 – 3.41 | ~16,500 | in step | the pen saturates white |
| 25 | 2.64 – 3.18 | ~49,900 | **24 tics behind** (the first backpressure) | asked 24, got 25 — I9's residual ±1 |

**The curve reading**: the FRAME COST IS FLAT — 8 → 25 moving hot lights all
land in the ~2.6–3.4 ms band, because the per-tile 8-slot light cap bounds
the shading work per texel no matter how many sources pile up. What grows —
superlinearly — is the EVICTION rate: with reach-16 circles in a ±12-tile
pen, most tiles see far more than 8 sources and every mover-move rebuild
evicts the excess (nearest-8 win, so the visual harm at this density is nil —
everything is saturated anyway). The costs that DID move: the WORKER fell 24
tics behind at 25 continuously-tripping pawns (I6's server-side signal —
movement pacing, not lighting), and the client's per-rebuild CPU bookkeeping.
Named successors for the mitigations the user deferred: the eviction rate as
a shorter-reach/authored-density question (a debug torch does not need reach
16), and the worker backpressure as a movement-throughput question — neither
is a lighting-render problem at these counts.

**Instrument verdicts** — `__lightcost(n)` turned out to be a SYNTHETIC bench
(it mints n fake lights and times the pass — 0.003–0.007 ms at n=8/16/24,
flat; it does NOT read the live scene, and it inflates droppedLights, so the
live rows exclude it). The I8 `glError 1282`: the ambient frame loop reads
CLEAN (getError 0 before and after); the 1282 is raised and consumed inside
the bench's own alloc/write/run sequence — benign, the hook's own artifact.

**Captures**: perf8 (8 wanderers, warm overlap), perf16 (the pen saturates),
perf24 (25 wanderers, full white flood).

**The restore** — cast wiped (the deathless debug torches go with it — F1's
no-needs design makes the wipe the ONLY cleanup, stated), wolves + bunnies
containers back: 1 wolf + 3 bunnies re-minted and adopted, seed guard quiet,
docs-check green, memory holds the findings (`torch-perf-delivered`). The
user's eyes close the stream.

## 2026-08-08 — P0/P1: the corpus + the torches brain

**P0** — `debug_torch` appended to things.toml (type pawn, subType `animal` —
no new subtype row needed; flat `#ffcf70` tint part; `walks` level 2 + the
constant `emit_light` level 1; NO needs) and the human_male drill bind
REVERTED (F4 — emit_light now lives ONLY on torch, torch_blue and
debug_torch, verified by grep). Load + content-check green; golden re-blessed
(the new kind + the reverted bind are the only diffs); 68 + 2 content tests
green.

**P1** — `brains/torches.rs` (the bunnies' mint/adopt/retry/wander loop minus
every need: Mind = at/dest/deadline; pace via the MERGED accessor; a fresh
move_to on every arrival or blown deadline; `NPC_TORCHES`/`NPC_HOME`/
`NPC_RADIUS` env, radius default 12 — the zoom-1 window, I5) + the main.rs
registry arm. npc compiles clean. The container lane:

    bin/sim run npc NPC_BRAIN=torches NPC_NAME=Torches NPC_HOME=100,50 NPC_TORCHES=<N>

Smoke-started: the brain boots, fetches the corpus, and fails CLEANLY on
"debug_torch: not in the definition registry" — correct, the running master
predates the kind; P2's scene reset reseeds it (the row-1 mint is the live
proof).
