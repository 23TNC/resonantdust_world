# Plan — `shard` (how we close current → design)

_Last updated: 2026-07-14._

## Done — the staged build (S0–S7)

The original path onto the [`design/`](../design/) is the staged plan in
[`stages/`](stages/) (S0 foundation → S7 tail). It's complete; the granular done-log
is [`work/spacetime-rewrite/completed.md`](../../../../../../work/spacetime-rewrite/completed.md).

With the 2026-07-14 conformance re-cut, **every identity/keying divergence is closed** (#2, #4, #5,
#10, #11) and the reference model is live-verified. What separates `current` from `design` now is
the **pipeline's shape**, not its vocabulary.

## Forward — the phases

The four remaining divergences, in the order we intend to take them and **why that order**. The
open-item detail (design vs code vs fix vs blast radius) lives in
[`current/divergences.md`](../current/divergences.md); this file holds only the **sequence + the
reasoning**, which is the part that otherwise dies with a session.

### P1 · Restore the gates, clear the deck — *cheap, do first*

- **The red test.** `dsl::loader::material_registry_and_packed_channels` fails at HEAD (pre-existing,
  unrelated) → [`work/spacetime-rewrite/issues.md`](../../../../../../work/spacetime-rewrite/issues.md).
  Until it's green `cargo test --workspace` cannot gate anything.
- **#3 · [`cleanup.md`](cleanup.md)** — delete the dead `cold_tiles` / `cold_things` / `experiment`
  modules + their `redeploy.sh` fam-entries. Pure removal; `design` never had them.
- **#9 · drop `Phase` / `priority`** from `shared/tick` (`domain.rs`, `actor_read_tic`). The design's
  ordering model is explicit `await`s + the read rule; the priority-DAG is vestigial.
  (`shared/tick` has no component folder yet — create it lazily if this grows past a deletion.)

**Why first:** as of 2026-07-14 *two of our three build gates don't gate* — the workspace `check`
skips the wasm's `js`-gated code (deviation **D-7**) and `cargo test` has been red at HEAD.
P2 is the largest refactor in the backlog; starting it without a working suite underneath is how a
D-3-class bug survives to the browser again. #3/#9 ride along because they're deletions.

### P2 · #1 · `event_log` → `actions : Vec<u64>` — the word DSL — *the main event*

Replace the fat struct (`actor_key`, `target_key`, `data0/1`, the named `*_server_reference`
columns) with the design's flat postfix word stream; `resolve_one` becomes the interpreter.

**Why now, not later.** The words are made of exactly the references we just re-cut —
`entity_reference`, `object_reference`, the `server_reference` qualification. That vocabulary is
settled and live-verified **today**. Build the stream on it now and it's built once; build it in
three months and it's another re-cut. The mirror of that argument: the edge and npc already write
to the fat struct, and every new caller widens the blast radius.

**Bound the scope.** The one genuinely open-ended piece is the *compile step* (surface plan →
`Vec<u64>`), which is unscoped and exactly the shape of thing that balloons. First cut: **hand-built
words for the existing verbs, no surface syntax, no compiler.** Let the compile step be its own
later decision once real word streams exist to compile *to*.

**Blast radius:** the worker's `resolve_one` (reads `actor_key`/`target_key` today) → interpreter,
reusing `shared/dsl`'s value-stack VM behind a `Vec<u64>` word decoder; the `append` signature; the
edge + npc callers.

### P3 · #6 lifecycle state machine + #7 drop barrier / holder GC — *adjacent to P2*

The status machine (`enqueue → queueing → in_queue → running → complete` / `queue_failed`),
idempotent re-drivable enqueue vs execute, the open-rows table, timeout eviction; plus the holder
refcount table, the master's `drop_timed_out` barrier before `bump`, and GC reduced to the trivial
zero-holder rule. The design calls this the biggest structural change and it is.

**Why after P2, and immediately after.** It's correctness scaffolding for a payload shape that is
about to change, and it lands in *the same worker code* P2 rewrites — sequencing them adjacent means
touching `resolve_one` once, deliberately, instead of twice.

**The dissent, recorded honestly:** a reasonable case says lifecycle *first* — crash-recovery is a
correctness hole and the event schema is "just" a representation. We're not taking it: hardening
recovery around a representation we already know is wrong buys scaffolding we'd then re-cut. If that
trade looks different later, change **this file** (with input) — don't silently reorder in code.

### P4 · #8 · Event/data shard split — *deferred*

`event_log` on event shards; `state`/`state_log`/`cold` + the holder table on data shards. Same
generic module, a **deployment** split. The design already calls it later, and #1's landing does not
depend on it.

---

The live execution state stays in the work-stream
[`docs/work/spacetime-rewrite/`](../../../../../../work/spacetime-rewrite/) — P1…P4 all fall out of
the rewrite, so they keep its folder rather than minting new ones. This file holds the **sequence +
reasoning**; [`todo.md`](../../../../../../work/spacetime-rewrite/todo.md) holds the executable cut
(**T-6…T-10**), and items move todo → remaining → completed as usual.
