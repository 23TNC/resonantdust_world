# Issues — spacetime rewrite

Genuine problems hit while building the shard component, with the **options** considered, the
**choice**, and **why** — the decision trail. Folded here from the old `docs/issues/00X` files
(2026-07-14). Newest work appends below.

| # | issue | phase | choice |
|---|-------|-------|--------|
| 001 | committing N targets in one `resolve` | S2 | `Vec<TargetState>` arg |
| 002 | regenerating worker/non-edge bindings | S3 | copy edge bindings (extend generator later) |
| 003 | `claim` refused same-worker re-claim (found live) | S3 | allow owner + unowned + expired |
| 004 | running worker/master binaries (no cargo/compose) | S3/S4 | container on host net → `rd run` |
| 005 | cold find-or-mint can't decode a generic payload | S6/S7 | worker decodes + payload-param `mint_cold` |
| — | hand-computed hex test inputs wrong (×2) | — | generate inputs from the real encoders |

---

## 001 — Committing N targets in one `resolve` (phase S2)

### Problem

A row is **atomic** and may touch several targets (an AoE; `a b MOVE ; c d MOVE`). The worker
computes *all* target effects in scratch and must commit them in **one** `resolve` reducer call
(same-shard = one transaction). But a SpacetimeDB reducer can't take a variable-arity argument
list, and the game payload is `decl_tick_pipeline!`-parameterized — so "write these N entities'
new states" has no obvious fixed signature.

### Options

- **A · `resolve(worker_reference, event_reference, results: Vec<TargetState>)`** where the macro
  emits `TargetState { entity_key, <payload…> }` as a `#[derive(SpacetimeType)]` struct. One
  atomic call; the reducer loops the Vec, writing each `state`/`state_log`. Type-safe.
- **B · Per-target sub-resolve** — the worker calls `resolve` once per target; the row completes
  when all targets are done. Simple fixed signature, but **not atomic across targets** (partial
  state is visible mid-row) and needs its own per-row completion barrier.
- **C · Pack all target states into a `Vec<u64>` blob**, decode in the reducer. Fixed signature,
  but loses type safety and re-implements payload (de)serialization by hand.

### Choice — **A**

`resolve(worker_reference, event_reference, results: Vec<TargetState>)`, with a macro-emitted
`TargetState`.

### Why

- **A preserves the row-atomicity invariant** ([lifecycle.md](../../components/server/spacetime/modules/shard/intent/lifecycle.md)):
  same-shard targets commit in one transaction, exactly as the design requires.
- **B breaks that invariant** — partial rows become visible and it reintroduces a per-row barrier
  we'd otherwise get for free from the single call.
- **C** duplicates the codec and throws away the schema's type-safety for no gain; the macro
  already parameterizes `state`/`state_log` by payload, so emitting one more struct with the same
  fields is trivial and keeps everything typed.
- Cross-shard targets are a *separate* concern — those use the convergent multi-call path
  (idempotent per `(source_shard, event_reference)`), which is orthogonal to how one shard's
  batch is committed.

### Status

Implemented in S2 (the `TargetState` struct + `Vec`-taking `resolve`).

## 002 — Regenerating worker (non-edge) bindings (phase S3)

### Problem

`rd build spacetime shard` runs `generate-bindings.sh`, which emits SDK bindings **only** to
`/workspace/game-server` (host `server/edge`). But the **worker**, **master**, and **npc** each
carry their *own* `src/bindings/shard/` copy, and those are stale after the S2 schema change —
the generator doesn't touch them.

### Options

- **A · Copy the regenerated edge bindings** to the other consumers. They're byte-identical (all
  `spacetimedb_sdk` client bindings for the same module — verified: `diff` of a stable file is
  identical pre-rewrite).
- **B · Extend `generate-bindings.sh`** to emit to every consumer dir (edge + worker + master +
  npc), or loop a consumer list.
- **C · Share one bindings dir** across consumers (a single `shard/` crate they all depend on).

### Choice — **A now, B later**

Copy `server/edge/src/bindings/shard` → the consumer for this phase; note that the generator
should grow a consumer list (B) so this isn't manual.

### Why

- **A is correct and immediate** — the bindings are genuinely identical, so a copy is exactly
  what regenerating would produce; it unblocks S3 without touching build infra mid-rewrite.
- **B is the right permanent fix** but editing `generate-bindings.sh` + the compose wiring is
  infra work orthogonal to the shard rewrite; deferred so it doesn't stall the phase.
- **C** (one shared bindings crate) is cleaner still but a bigger refactor — worth considering
  once the schema stabilizes.

### Status

S3: copied edge → worker bindings. `generate-bindings.sh` extension (B) tracked as follow-up.

## 003 — `claim` must allow the same worker to re-claim across phases (phase S3, found live)

### Problem

The two-phase lifecycle has the **same worker** claim a row **twice**: once `enqueue → queueing`,
then again `in_queue → running`. My first `claim` treated "free to claim" as *unowned OR
lease-expired* — so the second claim, where the row is still owned by that same worker (lease
fresh), was refused. The row stuck at `in_queue`; `resolve` (which requires `running`) then
no-op'd, and nothing reached `state`. Caught by the live `spacetime call` walk-through (R1), not
by compilation.

### Options

- **A · Allow the current owner to re-claim** — `free = unowned || owner == me || lease expired`.
- **B · Separate reducers per phase** — a distinct `claim_execute` that only advances `in_queue
  → running`, so a single "is it mine" check per transition.
- **C · Fold the transition into `stand_up`/`ready`/`resolve`** — no explicit second claim; the
  phase reducers advance status themselves and fence on the worker id.

### Choice — **A**

`free = worker_reference == NONE || worker_reference == me || lease expired`.

### Why

- **A is minimal and correct** — it's the natural fence semantics: a row is claimable by its
  current owner (continue), an unowned row (fresh), or an expired one (eviction/handoff). One
  reducer serves both phase transitions.
- **B** doubles the reducer surface for no real gain.
- **C** couples fencing into every phase reducer and muddies the clean claim→act split.

### Status

Fixed in the pipeline `claim` reducer; re-verified live (event → `complete`, `state` populated).

## 004 — Running the worker/master binaries for a full-stack test (phase S3/S4)

### Problem

The worker and master are native binaries with **no host cargo** and **no build/run compose**
(unlike the edge/gateway). The old `rd_master`/`rd_worker` runners point at deleted paths (the
stale-binary problem the memory warns about). So the full stack couldn't be run to prove the
pipeline drives itself — only the module could be poked via `spacetime call`.

### Options

- **A · A throwaway rust+openssl container on `--network host`.** Build the binaries inside it
  (deps cached in the mounted `target/`), run them in the background pointed at a test DB.
- **B · Add real compose files** for worker/master (like edge) and an `rd run worker/master`.
- **C · Cross-compile static binaries** (musl + vendored openssl) runnable on the host directly.

### Choice — **A** (for the test); **B** is the real fix

`docker run -d --name rd-run --network host -v $PWD:/w rust:slim sleep infinity`, then
`apt-get install -y libssl-dev pkg-config ca-certificates`, `cargo build` worker + master, and
`docker exec -d ... ./target/debug/{master,worker}` with `ST_URI=http://127.0.0.1:3000` +
`SHARD_DB=<db>` (`--network host` reaches the `spacetime-start` container's :3000).

### Why

- **A unblocks the full-stack test immediately** with no repo changes — the link just needs
  `libssl-dev` (spacetimedb-sdk → native-tls), and `--network host` gives the binaries the DB.
- **B is the permanent fix** (proper dev-stack integration) but is infra work beyond the rewrite.
- **C** is best for deploy artifacts but overkill for a dev smoke test.

### Result — full stack verified live

Real master (2 Hz, driving `drop_timed_out → bump`) + real worker (two-phase loop + DSL
interpreter) against a fresh shard:
- `append` a SPAWN → the worker **autonomously** claimed → stood up → readied → claimed →
  resolved → entity in `state` (kind 5, loc 17). Worker log: `resolved ev=1 targets=1`.
- `append` a MOVE on it → kind **carried forward** (5), location → 34, rotation → east (the
  interpreter's `facing_from_delta`).

The whole hot-path pipeline runs end-to-end with the actual binaries. Follow-up: add worker/master
compose + `rd run` (B).


### Resolution — option B shipped as `rd run` (light form)

`bin/lib/run.sh` adds `rd run <up|worker|master|stop|status|logs>`. Rather than the heavier
edge-style compose+up/down wiring, it keeps ONE persistent `rd-run-<env>` container
(rust:slim + libssl-dev, repo bind-mounted, `--network host`), builds the debug binaries into
the mounted `target/`, and runs them detached against `resonantdust-<env>-zone-0`. Logs land at
`server/{worker,master}/{worker,master}-<env>.log`. **Verified:** `rd run up` on dev drove an
appended spawn to a resolved `state` row (worker log `resolved ev=1`), master ticking. This is
the self-driving half of the dev stack; edge/gateway remain `rd up`/`rd deploy`.

## 005 — Cold `find-or-mint` can't decode into a generic payload (phase S6/S7)

### Problem

`find-or-mint` (promote a cold object to hot when a row targets it) must **write the payload
fields** of the new hot entity — its `kind`, `zone_id`, `location`, … taken from the cold
object. But `decl_tick_pipeline!` is payload-**generic**: inside the macro the payload is an
opaque `$( $pf : $pt )*` list, so the module *cannot* map "the cold object's kind" onto "the
`kind` field." (This is the same reason the old `unpack` reducer took the whole payload as
args — a reducer can't decode a cold object into a payload it doesn't understand.)

So `find-or-mint` can't live wholly inside a module reducer.

### Options

- **A · Worker decodes; module gets a payload-parameterised `mint_cold`.** The worker (which
  subscribes `cold` and *is* the spatial resolver) does the decode: on a **positional/cold
  target**, it looks up `state` for an existing hot entity at that location; if none, it reads
  the `cold` object, decodes its kind/position, and calls a macro-emitted
  `mint_cold(event_ref, positional_target, <payload…>)` reducer that seeds the hot entity +
  appends the `cold_removed` tombstone. Keeps the module generic; the decode is where the
  payload knowledge already is.
- **B · Payload-specific mint in the module** — break genericity: a spatial-only `mint_cold`
  hard-coding `kind`/`zone_id`/`location`. Simple but couples the generic engine to one payload.
- **C · Interpreter-side** — the mint stores a raw `object_kind_reference`, and the action's
  program decodes it into state. Pushes cold decoding into every plan; heavy.

### Choice — **A**

Worker decodes a positional/cold target and calls a payload-parameterised `mint_cold`.

### Why

- **A matches the design** ([hot-cold.md](../../components/server/spacetime/modules/shard/intent/hot-cold.md), decision that the
  worker does the game-logic decode; the module is a store) and keeps `decl_tick_pipeline!`
  payload-generic — the `mint_cold` reducer is emitted with `$pf` like `resolve`/`seed_entity`.
- **B** re-couples the engine to the spatial payload, undoing the generalization.
- **C** spreads cold-object decoding across the DSL for no benefit.

### Scope (the remaining S7 find-or-mint work)

1. Module: emit `mint_cold(event_ref, positional_target, <payload…>)` — mint a hot key, seed its
   resolved state from the args, append the `cold_removed` tombstone, and open the action's
   pending row against the minted key.
2. Worker: on a **positional** target (`entity_ref_is_positional`), check `state` by location
   (existing hot → reuse); else read `cold` + decode the kind at `(x,y)` and call `mint_cold`.
   (Worker subscribes `cold`.)
3. Edge: `interact` targets the cold object by a **positional** `entity_reference`
   (`pack_positional_entity(zone, location)`); enqueue promotes it.
4. Live-test the cold→hot promotion (interact on a seeded cold thing).

### Status

Design resolved; implementation is the next phase. Blocked nothing already shipped — the hot
path is fully live-verified; only cold `interact` remains stubbed.

## Extra

### hand-computed hex test inputs wrong (×2, 2026-07-14)
**Problem:** positional-target and `DAMAGE` `OBJECT` words hand-encoded incorrectly; the CODE was
right both times. **Choice:** generate all test inputs from the real `encode_*`/`pack_*` via a
throwaway test, never by hand. **Why:** recurring foot-gun; the codec is the source of truth.

### ✅ RESOLVED — `dsl::loader::material_registry_and_packed_channels` was red at HEAD (2026-07-14)
**Problem:** the workspace test suite has a **pre-existing failure** — `visual_for_def(2).unwrap()`
panics on `None` ([loader.rs:802](../../../shared/dsl/src/loader.rs)); the test's stone def no longer
resolves. Unrelated to the rewrite (`dsl` doesn't depend on `codec`; `shared/dsl` + `content` are
untouched). Confirmed by running `-p resonantdust-dsl --lib` in a clean worktree at HEAD.
**Impact:** 🟡 `cargo test --workspace` was never green, so "tests pass" couldn't gate anything —
each run needs the failure recognised and stepped over by hand, which is how a *real* regression
would slip by. See also **D-7**: `check` skipped the wasm's `js`-gated code. At the time, **two of
our three build gates didn't gate**. Both are fixed now (T-6 greened the suite; D-7's `check` is
two-pass).
**Resolved (T-6, 2026-07-14):** the **fixture** was wrong, not the loader — and the test was **born
red**: it fails identically at `d25b872`, the commit that added it, so it never passed and nothing
regressed. `node_visual` bails at [loader.rs:234](../../../shared/dsl/src/loader.rs) —
`store.read("prims.0.tint")?` makes a base tint **mandatory** for a node to have any visual — and the
fixture's stone sets `texture` + `packed.0.{material,tint}` but no `&tile.tint`. Every real def in
`content/visual/*.rd` sets one (identity `#ffffff` where the texture carries the colour), including
the real `::stone>` the fixture is a miscopy of (`#6b6b6b &tile.tint set`). Fixed the fixture to
match real content; the assertions are untouched and now pass. **`cargo test --workspace` is green
for the first time** (107 tests) — the gate works again.

**But the fixture's author assumed something real** (see the fork below): they expected a def to be
able to bind *only* packed channels. It can't, and the way it can't is silent.


### A tile/thing def that omits `&tint` silently loses its ENTIRE visual (2026-07-14)
**Problem:** `node_visual` reads the base tint with `?`
([loader.rs:234](../../../shared/dsl/src/loader.rs)), so a def whose `@on_create` exports a prim but
never sets `&tile.tint` yields `None` — **not** "a visual with no tint". Its **texture vanishes too**
(`tile_texture_stems` → `""` via `unwrap_or_default`), and `tile_packed_channels` → all-default. A
def that binds only packed channels disappears rather than rendering untinted.
**Impact:** 🟡 latent — no real content hits it (every def sets a tint), and it's how T-6's test was
born red: its author wrote a packed-only stone and reasonably expected it to work.
**Why it's shaped this way:** the `?` doubles as "did this node produce a prim at all?" — but
`run_node_hook(node, "visual", "on_create")?` on the line above **already** answers that. So the tint
`?` only guards "prim exists but has no tint", and conflates *absent* with *invalid*.
**Choice:** left as-is; T-6's scope was restoring the gate, and changing it is a **content-semantics
decision, not a cleanup** — logged in [forks.md](forks.md) (2026-07-14, "base tint: mandatory,
defaulted, or an error?") as undecided. Deliberately not settled while executing something else;
that's how D-3 happened.


### `current/divergences.md` #1 contradicted `design/event-dsl.md` (2026-07-14, caught in T-9 scouting)
**Problem:** divergence #1's *fix* line said to stand up the RPN interpreter by **"reusing
`shared/dsl`'s value-stack VM behind a `Vec<u64>` word decoder"**. The design decided the exact
opposite — [event-dsl.md](../../components/server/spacetime/modules/shard/design/event-dsl.md)
§"Not reusing `shared/dsl` (decided)": the event VM is **purpose-built in the worker**, because
`shared/dsl` is a text-parsed `.rd` VM for tile *visuals* (`Cell` trees of render prims) — different
syntax, value model, and purpose; the only overlap is the abstract postfix-stack shape, *"a few
lines, not a library"* — *"don't design around it."*
**Impact:** 🟡 caught before any code. I had already propagated the wrong instruction into T-9, so
the interpreter would have been built on the wrong foundation and in violation of an explicit
design decision — the exact failure mode the deviations discipline exists to prevent, arriving via
a **stale doc** instead of a careless edit.
**Choice:** `design/` wins (CONVENTIONS: `design/` is the final-state specification; `current/`
is a convenience snapshot). Corrected #1's fix line and T-9. **Lesson:** `current/` and `plan/`
notes are not authoritative — re-read `design/` at the start of each task, not the ticket.
