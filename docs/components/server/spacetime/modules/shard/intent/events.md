# The event log and the tic pipeline

The event log is where intent lands. Its intended shape and *where its processing runs*
are both different from the code today ([divergences.md](../current/divergences.md)); this doc is the
target.

## An event is an object ✅

Events are `type = event` objects — the log is just more objects. That's why `event` /
`server` / `player` are object types even though they aren't spatial.

## The event row — an event carries a DSL *program* ✅ (model) / ⚠️ (code)

> ✅ **DECIDED (supersedes the fixed-action model below).** An event does not carry a fixed
> `action:u16` + positional reference slots. It carries a **program** — a small DSL
> compiled into a `Vec<u64>` — and the **worker executes it**. "Actions" are *written* in
> the DSL, not enumerated in the engine.

```
event
  event_reference  : u32        auto-populated; a u32 so it can be passed as a reference
  tic              : u32
  actions          : Vec<u64>   the RPN word stream — operands + verbs (see event-dsl.md)
  worker_reference : u16        full server_reference of the worker assigned the job
  status           : u8
```

The `actions` field is a **flat postfix (RPN) word stream** — each `u64` is
`op_code:4 | reserved:12 | server_reference:16 | payload:32` (tag on top; low 48 bits a clean
`server + u32 ref`), where `op_code` is `LITERAL`/`OBJECT`/`ACTION`/`ALIAS` (constant / operand
/ verb / prior-action back-ref). Full worked example (`tile, object, MOVE`) and encoding in
**[event-dsl.md](../design/event-dsl.md)**.

- ✅ **The worker is the VM.** During `resolve`, the worker **interprets the event's
  `actions` program** against the operands it names, computes the new state, and commits. All
  action *meaning* lives in the program + the interpreter — **nothing is hardcoded per-action
  in the shard**, the worker grows no branch for a new verb, and the shard never runs the
  program.
- ✅ **References are operands.** `cold_reference` / `hot_reference` (u32) and
  `server_reference` (u16) are values the program pushes/reads — `u64` words carry them
  directly. There is no fixed slot schema and no cold/hot discriminator bit. A verb just names
  its target; if that target is a `cold_reference`, **enqueue has already minted it hot** by the
  time the verb runs (see [hot-cold.md](hot-cold.md)) — the interpreter only ever sees hot.
- ✅ **Why a program, not an `action` enum.** A fixed `action` needed a hardcoded slot schema
  *and* hardcoded worker logic per action, and every new verb touched the engine. A program is
  open-ended: it expresses guarded actions (operands, `await`, branch, `fail`) the worker
  interprets — and new actions are authored in the DSL without changing the engine. **One
  action vector per row** — the vector may hold *several* actions hitting *several* targets
  (e.g. an AoE) that resolve together. A step that must run *after* another (a cross-tic
  dependency) is a *separate* row chained by `await` ([event-dsl.md](../design/event-dsl.md) §Multi-step
  plans) — not one program resumed across tics.

### Decisions & remaining

- ✅ **No `shared/dsl` reuse.** `shared/dsl` is a text-parsed VM for tile *visuals* (`Cell`
  tree of render prims) — different syntax, value model, and purpose. The event VM is
  **purpose-built** ([event-dsl.md](../design/event-dsl.md)).
- ✅ **Routing is not opaque.** Each operand word carries its own `server_reference`, and the
  **requesting server designates the target** (which entity gets a pending row) when it appends
  the row — so the shard/worker never has to parse-to-route.
- ✅ **Determinism & termination.** Deterministic ops; bounded by the forward-only `SKIP`
  branch (no backward jumps); `TIMEOUT` measured in **tics**, no wall-clock / ambient reads.

<details><summary>Superseded: the fixed-action model (kept for context)</summary>

Earlier the event carried `action:u16` + `object_reference:Vec<u32>` +
`server_reference:Vec<u16>`, where the action defined each slot positionally (`move` →
`[0]` pawn `[1]` tile, `mint_hot` → `[0]` cold crate, …). The DSL program subsumes this: an
action's "slots" are just the operands its program reads, and the positional schema is
whatever the program expects — without the engine holding a table of per-action schemas.
</details>

## How it resolves — see [lifecycle.md](lifecycle.md)

The full resolution story — the two-phase recoverable lifecycle, the drop-on-miss causality,
the event-shard/data-shard split, and crash recovery — lives in its own doc,
**[lifecycle.md](lifecycle.md)**. The essentials as they touch the *log*:

- **The shard is a dumb store + metronome + fence; the worker does the game logic.** Shard
  reducers (`append`, stand-up, `resolve`, `tick_gc`) never execute the DSL — the worker VM
  does, in scratch, then asks SpacetimeDB to write.
- **The row is the unit of work, and it is atomic** — a worker computes *all* a row's target
  effects and commits them together (same-shard = one ST transaction; cross-shard = convergent
  via idempotent re-drive, [lifecycle.md](lifecycle.md)).
- **Two phases, both recoverable:** *enqueue* (stand up the target `state_log` rows, with a
  window-checked) then *execute* (run the vector, commit results). A durable `status` on the
  row (`enqueue → queueing → in_queue → running → complete`, or `queue_failed`) makes every
  phase crash-recoverable — re-drive until `resolved`.
- **The gap is `+3`** now (was +2): the recoverable enqueue phase adds a tic → ~1.5–2 s
  request-to-visible at 2 Hz (client prediction hides it for player-facing actions).
- **Worker-triggered rows** queue at tic+2 or later; dedup is structural (see lifecycle.md).
