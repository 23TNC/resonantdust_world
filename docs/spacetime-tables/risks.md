# Risks & trade-offs

The design is coherent, but it's an ambitious distributed system built on SpacetimeDB. This is
the honest ledger of what could bite, what's a deliberate trade, and what's already been closed.
Ranked by how much attention they deserve.

## Act on these first

- **R1 · It's all on paper — build a vertical slice before the stages.** Everything is
  ✅-decided; none is validated against SpacetimeDB (round-trip counts, subscription/re-send
  cost, transaction semantics, throughput). Before S0–S7, prove the risky loop end-to-end for
  **one shape, one action**: enqueue → `find-or-mint` → execute → convergent write → **kill the
  worker mid-write** → confirm re-drive converges. If the substrate fights this, learn it in week
  one, not stage six. *(Recommended: an "S-slice" stage before S0.)*
- **R2 · Determinism is load-bearing and fails silently.** Recovery/convergence assume
  re-execution is bit-identical. Violated by `HashMap` iteration order (the current `bump` uses
  one), any RNG, wall-clock, cross-platform float. A single nondeterministic op → re-drive
  produces *different* state, and *nothing errors*. Guard actively: ordered maps, seeded/no RNG,
  no wall-clock in resolve, and a **determinism test** (resolve twice, diff).

## Deliberate trades (chosen, not bugs)

- **T1 · Dropping is load-bearing for correctness → hard real-time deadline.** Replacing the
  watermark with drop-on-miss means each stage has a ~1-tic (500 ms) budget; a missed action is
  *lost* (like dropping late frame input). Consequences: worker latency directly causes action
  loss; **`queue_failed` is a first-class client outcome** (retry / tell the player), not a
  corner; capacity (workers/shard clearing each tic) is a hard constraint. Right call for a
  real-time sim — but go in eyes-open.
- **T2 · ~1.5–2 s pipeline latency, compounding across steps.** +3 is best case; a multi-step
  plan sums (`move` + walk time, then `inspect` +3). Fine for world/NPC sim; player-facing
  actions need client prediction of *outcomes*, not just movement. Confirm by *feeling* the
  round-trip, not trusting the tic math.
- **T3 · Read-holds add write traffic (if used).** Refcounting reads costs an acquire/release per
  read — bounded to the in-flight working set, not the watermark's hot-entity hotspot, but real.
  Cheaper equivalent: because ≤ T−1 bounds reads to a small window near the frontier, GC keeping
  `{latest} ∪ {last-few-tics} ∪ {dirty}` is already correct *without* per-read holds. Trade:
  explicit holds = robust/future-proof; bounded-window = cheaper. Writes are tracked by `dirty`
  either way. *(Lean: bounded-window for reads unless a deep-read feature appears.)*
- **T4 · The DSL front-loads a runtime for future authorability.** A bytecode VM buys authoring
  new actions without engine changes, but costs a compiler, a VM, cross-shard/cross-tic bytecode
  debugging, and op-versioning — for a current set of ~move/inspect/attack. A conscious "we want
  the generality now" choice, not a default.

## To verify (small, not blockers)

- **V1 · Cross-shard tic sync.** The drop-on-miss causality assumes a globally consistent tic.
  The master is the sole authority and rows are tic-stamped at enqueue, so skew is
  liveness/tuning, not correctness — but keep the master's bump fan-out tight and bound the skew.
- **V2 · Drop cascade = N independent timeouts.** A dropped row that others awaited must let them
  time out and back out locally (no active propagation). The mint-at-enqueue absorption removes
  the "B depends on A to create B's target" case, so the only coupling is the execute-time
  `await` + its timeout. Verify a dropped-dependency chain unwinds cleanly.
- **V3 · GC observability & invariants.** GC is now trivial (zero-holder + not-latest + old), but
  write the invariant down and assert it; a leaked hold is safe (wasteful), a premature release is
  corruption.
- **V4 · `PACK` trigger owner.** Decide who enqueues `PACK` (edge / master / periodic sweep).

## Unaddressed / deferred — game-server scope

Whole areas the shard/DSL design doesn't cover yet, with their disposition. These aren't
pipeline bugs; they're the rest of the game server.

- **A1 · Authority / validation / anti-cheat** — nothing yet decides whether an action is
  *legal* (ownership, range, cooldown, resources), and if clients could submit raw programs
  that's a huge attack surface. **Disposition:** the **edge** owns this — it's the trusted
  composer/validator (clients send intents, the edge validates and emits the program). Deferred,
  edge-owned.
- **A2 · Where game rules live — ✅ decided: in `shared`.** Verb effects (movement rules, damage
  formulas, …) live in a **shared crate**, so the **worker** (execution), the **edge**
  (validation/composition), and the **client** (prediction) all run the *same* code. One
  implementation → client prediction matches server execution by construction, and the "generic
  worker" stays generic in *scheduling* while the effects are shared, deterministic Rust. See
  [event-dsl.md](event-dsl.md).
- **A3 · Master is a single point of failure** — sole tic authority + drop barrier; if it dies,
  the world freezes. **Disposition:** deliberate — a **single unified non-distributed authority**
  is what *provides* the guarantees (one tic, deterministic drop, total ordering); without it we
  couldn't guarantee them. **Redundancy/failover is a later concern**, layered on, not now.
- **A4 · Autonomous simulation & NPC scale** — the pipeline is reactive; a living world needs
  autonomous events and many deciding agents, which flood a budget-limited pipeline.
  **Disposition:** today **npc-player drivers control *groups* of NPCs** (batched); a more
  scalable/server-side driver will likely be needed **eventually** — deferred.
- **A5 · Fairness / starvation** — ❓ workers pull work with no priority or fairness, so
  drop-on-miss under load drops *arbitrary* actions; a player in a busy zone could be
  systematically starved. Needs a fairness/priority policy so degradation is graceful. **Open.**
- **A6 · Content/registry versioning vs. stored data** — ❓ `kind_id`/`subtype_id` are
  content-derived and baked into long-lived `cold` zones; a content change must not silently
  re-mean stored `object_kind_reference`s. Append-only ids help, but the stored-data-vs-evolving-
  registry contract isn't written down. **Open.**

## Resolved (closed by later design)

- ~~Cross-shard multi-target atomicity~~ → **convergence** (idempotent re-drive), not 2PC.
- ~~Watermark write-amplification / hot-entity hotspot~~ → **removed**; drop-on-miss makes the
  watermark redundant (strict staging ⇒ writes only at the frontier, reads only of the sealed
  past).
- ~~GC as a correctness hotspot~~ → **refcounted holder table**; GC makes no correctness decision.
- ~~`read_rule`/`priority` survival~~ → keep `read_rule`, drop `priority`/`Phase`.
