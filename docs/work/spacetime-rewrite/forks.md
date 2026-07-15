# Forks — spacetime rewrite

Decision points with more than one viable path + which we chose + why. Chronological.

- **2026-07-14** · **Re-cut split along the hot/cold boundary** — the codec re-cut hit the
  geometry conflict ([blockers.md](blockers.md) B-2). Options for the whole re-cut: **(A)** land it
  all (needs the world-geometry call); **(B)** split — land the geometry-free *hot/identity/event*
  side now, defer the *cold* side to B-2; **(C)** stop. **Chose B.** **Why:** the hot side (`#5`
  `hot_reference`, `#10` `server_reference`, `event_reference` u32 — the "big PK change") is fully
  settled and geometry-free; the cold side (`#4`, compressed `position_reference`/`cold_reference`
  addressing, the `cold` `macro` header) is the *only* part that needs B-2. Splitting there lands
  the bulk without unilaterally deciding world geometry (the B-1-class failure).
- **2026-07-14** · **`entity_key` column width under `#5`** — options: **(a)** narrow the
  `state`/`state_log` PK to `u32 hot_reference`; **(b)** keep `u64`, holding the new-layout
  `entity_reference` (`reserved:10 | reference_id:6 | server_reference:16 | object_reference:32`).
  **Chose (b).** **Why:** "`object_reference = hot_reference:32` **server-qualified**" *needs* the
  16-bit `server_reference` alongside the 32-bit handle — that's the u64 `entity_reference`.
  Narrowing to u32 drops server qualification (breaks cross-shard identity + the `home_shard`
  routing the worker does by `server_reference`). Keeping the u64 realizes `#5` faithfully and is
  functional-neutral (only the *layout inside* the u64 changes), minimizing churn (no column
  rename → bindings/consumers unchanged in shape).
- **2026-07-14** · **Hot vs cold `entity_reference` variants** — the new `object_reference:32` can't
  hold a world-global `zone_id:u32`+cell+layer (that's the B-2 geometry). So the two identity
  classes coexist in the one `u64` `entity_key`, discriminated by `reference_id`: **HOT** =
  `reference_id:6 | server_reference:16 | object_reference(hot):32` (clean new layout); **COLD**
  (find-or-mint/Interact targets) = `reference_id:6 | position(zone_id:32|location:8|layer:8):48`
  (world-global, pre-B-2 compat). The worker branches on `reference_id` (was: the old
  `entity_type` positional top-bit). Documented as the interim cold form until B-2 settles.
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

- **2026-07-14** · **Base `&tint`: mandatory, defaulted, or an error?** — **UNDECIDED, deliberately.**
  Surfaced by T-6: `node_visual` reads the base tint with `?`, so a def that exports a prim but sets
  no `&tile.tint` silently yields **no visual at all** — texture included. Options: **(a)** keep it
  mandatory (status quo; matches the real-content convention that every def sets a tint, identity
  `#ffffff` when the texture carries the colour); **(b)** default the tint (`0`/white) so a def can
  bind only packed channels — what the test's author assumed; **(c)** make the omission a **load
  error**, so it fails loudly instead of rendering nothing. **Not chosen** — T-6's scope was
  restoring the build gate, and this is a content-semantics call with a real trade (b is friendlier
  to the packed-channel model; c is honest; a is what all content already does). Deciding it while
  executing an unrelated task is precisely the D-3 failure. Detail: [issues.md](issues.md).

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
