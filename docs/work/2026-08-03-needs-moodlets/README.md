# Needs & moodlets — the pawn feels, the panel shows feelings — 2026-08-03

_Components: `shared/dsl`, `server/spacetime` (+ edge/worker verbs), `client/npc`,
[`client/webgl`](../../components/client/webgl/). Plan in [`todo.md`](todo.md); decisions in
[`forks.md`](forks.md)._

## The user's call

> "Our pawns will have a set of needs like rimworld, and they will display a set of moodlets like
> sims4. So while a pawn might have a bladder need, like prison architect we wont display that
> directly and instead display moodlets like sims4 does. I believe this extra layer of abstraction
> will help make the game feel more alive and less like a spread sheet manager." — 2026-08-03

> "moodlets will, like sims4, be temporary modifiers to moods and stats. We will display the
> current moodlets in our details panel. We will define needs and moodlets using our dsl. The
> first need I'd like to implement is thirst." — 2026-08-03

> "npc will then take these moodlets into consideration when driving actions. We will implement an
> action system in the next turn … add a drink action and the pawn would use drink on a water tile
> to satisfy thirst." — 2026-08-03

## The model

**Needs are hidden state; moodlets are the display and the decision surface.** A need is a 0..1
SATISFACTION that depletes toward zero over tics ([F1](forks.md#f1) — resolving the user's open
"increase or decrease" question: every need falls, bad states are LOW, one dialect for every
future need). A moodlet is a DSL-authored consequence: conditional moodlets are bands on a need's
satisfaction and are **derived, never granted** — a pure function of `(need row, tic, corpus)`
that every observer computes identically ([F2](forks.md#f2)); timed moodlets (post-drink
"Quenched") are stored grants whose lane is built now and exercised by the action stream. Mood is
one scalar: `clamp(base + Σ active moodlet offsets)` ([F5](forks.md#f5)).

**Nothing ticks per tic.** A need row is `(satisfaction, set_tic)`; the current value and every
band-crossing tic are computed from the DSL depletion rate ([F4](forks.md#f4)) — the same posture
as movement speed in TICS/TILE and the learned tic↔wall estimate. The row is written only when
something HAPPENS (mint, drink, forced set).

**One evaluation implementation** ([F3](forks.md#f3)): rust in a shared crate — npc imports it,
the client calls it through `shared/wasm`. Two copies of a band comparison is the drift class this
repo has already paid for twice.

**Authoritative where the pawn lives:** need rows and stored moodlets are PAWN-SHARD lanes, not
npc-local state — an npc restart must not reset the wolf's thirst; shards are the root of truth.

## Non-goals (the successor stream's charter)

The ACTION system — drink on a water tile, need restoration, timed-moodlet grants from actions —
is explicitly next turn. This stream ends with the wolf's Brain SEEING its moodlets in the
decision context and logging band transitions; its behaviour stays the A→B trips. No need bars
appear anywhere in the UI, this stream or ever — the abstraction IS the feature.

## Exit

A wolf runs from full to Dehydrated on a drill-scaled depletion while doing its normal trips; the
details panel and the npc log flip moodlets at the SAME computed crossing tics with zero events in
between; the user's eyes close the stream.
