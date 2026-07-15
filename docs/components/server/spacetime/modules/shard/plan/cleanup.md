# Plan — retire the dead cold/experiment modules ✅ **DONE (2026-07-14, T-7)**

_Done — kept as the record of what was actually there. Last updated: 2026-07-14._

> **What we found on execution.** This plan (and divergence #3) overstated the leftover. The module
> **sources** were already deleted by `2b2fc58` / `d47f152` — only `chat`/`index`/`players`/`shard`
> have been tracked since — and with no `Cargo.toml`, `rd_list_modules` never treated the three as
> module units, so they were **not** being built or deployed. What actually remained was ~530MB of
> untracked `target/` + `Cargo.lock` detritus on disk, plus the dead `fam` arms below.
> Step 1's `git rm` was therefore a no-op (nothing tracked); step 2 was the real change.

## What

Delete three dead SpacetimeDB modules and their build wiring:

- **`cold_tiles`**, **`cold_things`** — retired in div #3 (commits `2b2fc58`, `d47f152`) and
  superseded by the `shard` module's `cold` table (the object model). The connectors/bindings
  were removed then, but the **module dirs + `redeploy.sh` fam-entries** (`cold_tiles) fam="cold-tiles"`,
  `cold_things) fam="cold-things"`) were left behind.
- **`experiment`** — a sync-experiment leftover; no live build references.

## Why here (plan) and not in design

They are **not part of the target** — `design/` describes the shard with its `cold` table and
never mentions them. They only exist in `current/` (reality on disk). Removing them is how we
close that gap, so the *action* is a plan item; the *target* (their absence) is already the design.

## Steps (when we resume coding)

1. `git rm -r server/spacetime/server/modules/{cold_tiles,cold_things,experiment}`.
2. Remove the `cold_tiles) / cold_things)` fam arms in `bin/lib/redeploy.sh` (+ any
   `default_cold_tiles_db()` / `default_cold_things_db()` and config references).
3. Grep for lingering references (`grep -rn cold_tiles\|cold_things\|experiment`) and clear docs
   that describe them as live (e.g. `docs/data-shards.md` is the legacy design — mark or retire).
4. `rd redeploy` dry-run to confirm nothing still maps to those families.

No behavior change — they're already unused; this just removes dead weight + the stale confusion.
