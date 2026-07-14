# 006 — Next tranche after the behavioral core (fork)

## Situation

The full-design pass closed **section A** of [remaining.md](../spacetime-implementation/remaining.md)
— every behavioral mechanism (composition+gating, RUNNING recovery, actor-reads+read-rule,
convergent cross-shard writes, control flow) is built + verified (`full-design 1..5`). What's left
is a genuine fork.

## Options

- **A · Browser integration (section C).** Wire the new `state`/`cold` pipeline into a runnable
  dev stack (worker/master) + the pixijs client, so the game renders on the new schema.
  *Unblocked — needs no new design decisions.* Large but mechanical/wiring.
- **B · Representation re-keys (section B: #4 region_zone, #5 hot_reference u32, #10 geographic
  server_reference) + PACK trigger.** Functional-neutral. **Blocked:** every one turns on the
  **object-model taxonomy** (kind/type/variant, D3 hot_reference), which the docs mark *in-flux*
  ([object-model.md] "4 open decisions", D3 "deliberately deferred"). Executing = unilaterally
  settling design the user reserved.
- **C · Stop & review.** Pause for the user to review the six commits.

## Choice — **A (browser integration)**

## Why

- **It's the only unblocked path.** B requires resolving explicitly-in-flux object-model
  decisions — settling those without the user is exactly the failure that triggered this whole
  full-design pass ("you simplified the design we spent hours worked on"). Reserve B until the
  taxonomy is settled.
- **It makes the proven pipeline real + visible.** The server pipeline is verified with raw
  reducer calls + real binaries; integration turns that into a running, in-browser game — the
  verification surface the design most values, and the thing that surfaces the *next* real
  problems (which then feed B's priorities).
- **C stalls without cause** — per [[decide-and-proceed]], don't pause on an unblocked fork.

## Plan (A)

1. Map the current wiring: what edge/gateway/npc/pixijs assume vs. the new `state`/`cold`/
   `event_log` schema. (scout first)
2. Stand up worker+master as a repeatable dev stack (issue 004 option B, or promote the
   throwaway-container recipe into `rd`).
3. Wire the client read path (subscribe `state`/`cold`, render) + the write path (edge builds DSL
   word streams — already `vm::encode_*`).
4. Verify end-to-end in the browser.

Sub-forks/blockers encountered during A get the same treatment (options → choice → why → execute),
logged here or in a follow-up issue.
