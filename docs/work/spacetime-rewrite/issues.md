# Issues — spacetime rewrite

Problems hit during execution + candidate solutions + which we chose + why. Chronological.
Detail for the early ones lives in `docs/issues/00X` (to be folded in fully during the component
migration); newer issues are captured inline here.

- **2026-07-13** · **001 · Multi-target resolve** — a row can write several targets; a
  per-target reducer call breaks atomicity. **Chose:** one `resolve(Vec<TargetState>)` committing
  the whole row in one transaction. **Why:** a row is atomic (same-shard = one ST txn).
  · detail: `docs/issues/001-multi-target-resolve.md`
- **2026-07-13** · **002 · Worker binding regen** — `rd build spacetime shard` emits SDK bindings
  only to the edge dir; worker/master/npc carry stale copies. **Chose:** copy edge→consumer per
  phase (A), extend the generator later (B). **Why:** bindings are byte-identical; unblocks now
  without infra work mid-rewrite. · `docs/issues/002-worker-binding-regen.md`
- **2026-07-13** · **003 · Claim same-worker re-claim** (found live) — events stuck at `in_queue`
  because `claim` refused a same-worker re-claim across phases. **Chose:** `free = unowned ||
  owner==me || lease-expired`. **Why:** a worker must be able to advance its own row. · `…/003`
- **2026-07-13** · **004 · Running worker/master** — no host cargo, no run compose. **Chose:** A
  now (throwaway `rust:slim --network host` + libssl), B later (real standup). **Why:** unblocks
  the full-stack test with no repo change. **→ resolved** by `rd run` ([completed.md](completed.md)
  #6, option B light form). · `docs/issues/004-running-worker-master.md`
- **2026-07-13** · **005 · Find-or-mint payload decode** — the module is payload-generic, so it
  can't decode a positional target's kind. **Chose:** the worker decodes; module `mint_cold` takes
  the decoded params. **Why:** keeps the shard from running the DSL; worker already has the codec.
  · `docs/issues/005-find-or-mint-payload-decode.md`
- **2026-07-14** · **hand-computed hex test inputs wrong (×2)** — positional-target and DAMAGE
  OBJECT words hand-encoded incorrectly; the CODE was right both times. **Chose:** generate all
  test inputs from the real `encode_*`/`pack_*` via a throwaway test, never by hand. **Why:**
  recurring foot-gun; the codec is the source of truth.
