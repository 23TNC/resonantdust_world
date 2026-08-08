# Forks — mover perf

_A choice I resolved, with what was rejected and why. A fork is mine; a
[blocker](blockers.md) is the user's._

## F1 — debug_mover is debug_torch minus the light {#f1}
_2026-08-08 · resolved at plan time_

**Chosen.** `type = "pawn"`, subType `animal`, one flat-tint part (a plain
gray so the eye separates it from the torch), `walks` level 2, NO needs, NO
emit_light. Everything else identical to debug_torch — same pace, same pen,
same brain — so the ONLY variable against torch-perf's rows is the light.

**Why walks 2 again**: comparability. A different pace would change the hop
event rate and break the lit-vs-dark comparison. **Rejected — reusing
debug_torch with the light bind removed**: that would retroactively change
torch-perf's kind and rot its recorded rows; a second kind is two TOML lines.

## F2 — the torches brain CONVERTS to a generic `debug` brain {#f2}
_2026-08-08 · the user's call ("convert the npc light mover module into a debug module"), adopted_

**Chosen.** `brains/torches.rs` RENAMES to `brains/debug.rs`; the hardcoded
`"debug_torch"` kind becomes `NPC_KIND` (default `debug_mover`), the count
env generalizes to `NPC_COUNT` (`NPC_TORCHES` dies with the old name), and
the main.rs arm registers `debug`. One parameterized debug harness — the
torch procession is `NPC_KIND=debug_torch`, this stream's is
`NPC_KIND=debug_mover`, and the next debug kind costs zero brain code.

**Why**: delete-don't-deprecate — two near-identical brains is drift waiting
to happen, and the I9 gate fix should exist in exactly one place. **Rejected
— keeping `torches` as an alias arm**: an alias IS deprecation.

## F3 — the primary metric is master−worker LAG OVER TIME, from the compose lines {#f3}
_2026-08-08 · resolved at plan time — the lesson of torch-perf's one sample_

**Chosen.** The worker's own log carries the pair per compose
(`composed component tic=X master=Y`); the sampler is a timestamped log read
every ~10 s across the soak — no new tooling, no code. Per row: ≥3 minutes
for 8/16, **≥10 minutes for 24** (the row where the ceiling lives). The
verdict per row is one of: IN STEP (lag ≈ pipeline constant), STABLE-BEHIND
(fixed offset — a latency, survivable), GROWING (an unbounded deficit — the
real ceiling). Beside it: the per-group `events=` distribution, the master's
pacing report, and the client `__framecost` spread at zoom 1 as the
secondary.

**Rejected — a committed lag-probe tool**: the log already prints both
clocks; tooling would be ceremony. **Rejected — client-first metrics**:
torch-perf proved the client flat; the open question is the worker.

## F4 — torch-perf's rows are the LIT TWIN; the comparison is the free finding {#f4}
_2026-08-08 · resolved at plan time_

**Chosen.** Same counts (8/16/24), same pen (±12 around 100,50), same pace
(walks 2), same protocol — so dark-vs-lit reads straight across the two
streams' tables. Expected: the worker lag is IDENTICAL (the worker never
touches lighting), which would cleanly pin torch-perf's lag on movement
throughput. If dark movers lag LESS, something server-side touches lights
after all — a finding nobody predicted, which is exactly why the row is
cheap insurance.

**Caveat, stated**: torch-perf's row 3 ran N=25 (the I9 overshoot) and
sampled lag once; the comparison at 24 is approximate. If the numbers sit
close to a boundary, re-run the lit row under this stream's protocol rather
than argue from stale data.
