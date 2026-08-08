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

## 2026-08-08 — P1/P2: the rows, and a verdict nobody predicted

**Protocol** — reset per row-group (pawn wipe, wolves+bunnies stopped, seed
guard quiet at 237), zoom-1 client foreground as the secondary, the lag
series extracted from the worker's own timestamped compose lines
(`tic=X master=Y`) — the full series, not samples.

| population (actual) | soak | worker lag: med / p90 / max | lag trend (means per slice) | events/group med / max | client frame ms |
|---|---|---|---|---|---|
| 8 dark  | 200 s | 4 / — / 57 | −0.6 → 13.2, oscillating | 1 / 28  | 1.02–1.34 |
| 16 dark | 200 s | 3 / — / 77 | one 64-tic excursion, RECOVERED | 1 / 96  | — |
| 24 dark | 11 min | **1 / 22 / 66** | 9.2 → 4.0 across tenths, NO trend | 1 / 164 | 1.32–1.48 |
| 24 LIT (the F4 twin, re-run) | 10 min | **1 / 13 / 73** | 1.1 → 2.0 across tenths, NO trend | 1 / 256 | (torch-perf: 2.6–3.4) |

Zone spread at 24: {82, 83, 98, 99, 115} — five zones (I3).

**The verdict — IN STEP at every count.** The user's predicted headline ("we
will likely catch our workers falling behind") did NOT materialize: over
10-minute soaks the worker holds 24 movers — dark OR lit — at a median lag
of ONE tic with zero growth. The lag is BURSTY: transient excursions to
~60–80 tics (correlating with large event batches — max 256 events in one
group, queue-fan and arrival clusters) that recover within seconds.
**Torch-perf row 3's "24 tics behind" was one of these bursts caught by a
single sample** — exactly the one-sample sin this stream existed to correct;
that row's lag claim is hereby superseded. Dark-vs-lit server-side:
IDENTICAL, as designed (the worker never touches lights). Client-side the
light delta is clean: ~1.0–1.5 ms dark vs ~2.6–3.4 ms lit at every count.

**What this buys**: the platform (`NPC_BRAIN=debug NPC_KIND=… NPC_COUNT=…`)
and the ruler (the compose-line lag series). The worker's real ceiling is
ABOVE 24 concentrated movers — finding it is a successor's job (push N up
with this harness until the trend appears); the burst mechanism (what queues
256 events in one group, and whether the spikes matter at all) is the other
named successor. Nothing here needed improving yet — which is itself the
measured, honest answer.
