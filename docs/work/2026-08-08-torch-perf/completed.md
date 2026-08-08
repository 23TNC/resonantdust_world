# Completed — torch perf

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
