# Completed — mover perf

## 2026-08-08 — P0: the kind and the harness

**content** — `debug_mover` appended (type pawn, plain `#b8b8b8` part, walks 2,
NO needs, NO light — debug_torch minus the light, F1). Load + content-check
green; golden re-blessed (the new kind is the only diff); 68 + 2 tests green.

**the harness** — `brains/torches.rs` → `brains/debug.rs` (git mv): the kind
is `NPC_KIND` (default `debug_mover`), the count `NPC_COUNT`; the `torches`
arm and `NPC_TORCHES` are GONE (grep confirms — only a doc-comment cites the
history); main.rs registers `debug`; npc compiles clean. THE harness line
(I8):

    bin/sim run npc NPC_BRAIN=debug NPC_KIND=debug_mover NPC_COUNT=<N> NPC_NAME=Debug NPC_HOME=100,50

(the lit twin swaps `NPC_KIND=debug_torch`).
