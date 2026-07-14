# Forks — spacetime rewrite

Decision points with more than one viable path + which we chose + why. Chronological.

- **2026-07-14** · **Next tranche after the behavioral core** — with Section A (behavioral
  mechanisms) all closed, the options were: **(A)** browser integration, **(B)** the
  representation re-keys, **(C)** stop & review. **Chose A.** **Why:** A was the only *unblocked*
  path (B needs the in-flux object-model decisions — see [blockers.md](blockers.md) B-1, and
  deciding them unilaterally is the failure that started this pass); A makes the proven pipeline
  real + visible and surfaces the next real problems (it caught the `home_shard` mint_server=0
  bug). · full detail: **006** below.
- **2026-07-14** · **Worker/master standup shape** — **(B-full)** mirror edge's compose +
  up/down/logs/deploy wiring; **(B-lite)** a thin `rd run` over a persistent container;
  **(B-minimal)** doc-only recipe. **Chose B-lite.** **Why:** least surface, directly matches the
  proven throwaway recipe; edge's heavier pattern is overkill for two debug binaries. ·
  → `rd run` in [completed.md](completed.md) #6.
- **2026-07-14** · **How npc pauses** — **(a)** relay the `paused` flag to clients (npc gates on
  `Event::Paused`); **(b)** npc infers pause from a frozen tic in state traffic. **Chose (a).**
  **Why:** authoritative + direct; the shard's `tic_meta` is the natural broadcast medium (edge
  relays it per-subscriber), and (b) is fragile (can't tell paused from idle). · → `/pause` in
  [completed.md](completed.md).

---

## Detail

## 006 — Next tranche after the behavioral core (fork)

### Situation

The full-design pass closed **section A** of [remaining.md](remaining.md)
— every behavioral mechanism (composition+gating, RUNNING recovery, actor-reads+read-rule,
convergent cross-shard writes, control flow) is built + verified (`full-design 1..5`). What's left
is a genuine fork.

### Options

- **A · Browser integration (section C).** Wire the new `state`/`cold` pipeline into a runnable
  dev stack (worker/master) + the pixijs client, so the game renders on the new schema.
  *Unblocked — needs no new design decisions.* Large but mechanical/wiring.
- **B · Representation re-keys (section B: #4 region_zone, #5 hot_reference u32, #10 geographic
  server_reference) + PACK trigger.** Functional-neutral. **Blocked:** every one turns on the
  **object-model taxonomy** (kind/type/variant, D3 hot_reference), which the docs mark *in-flux*
  ([object-model.md] "4 open decisions", D3 "deliberately deferred"). Executing = unilaterally
  settling design the user reserved.
- **C · Stop & review.** Pause for the user to review the six commits.

### Choice — **A (browser integration)**

### Why

- **It's the only unblocked path.** B requires resolving explicitly-in-flux object-model
  decisions — settling those without the user is exactly the failure that triggered this whole
  full-design pass ("you simplified the design we spent hours worked on"). Reserve B until the
  taxonomy is settled.
- **It makes the proven pipeline real + visible.** The server pipeline is verified with raw
  reducer calls + real binaries; integration turns that into a running, in-browser game — the
  verification surface the design most values, and the thing that surfaces the *next* real
  problems (which then feed B's priorities).
- **C stalls without cause** — per [[decide-and-proceed]], don't pause on an unblocked fork.

### Plan (A)

1. Map the current wiring: what edge/gateway/npc/pixijs assume vs. the new `state`/`cold`/
   `event_log` schema. (scout first)
2. Stand up worker+master as a repeatable dev stack (issue 004 option B, or promote the
   throwaway-container recipe into `rd`).
3. Wire the client read path (subscribe `state`/`cold`, render) + the write path (edge builds DSL
   word streams — already `vm::encode_*`).
4. Verify end-to-end in the browser.

Sub-forks/blockers encountered during A get the same treatment (options → choice → why → execute),
logged here or in a follow-up issue.
