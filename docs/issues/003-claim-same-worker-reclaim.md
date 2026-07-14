# 003 — `claim` must allow the same worker to re-claim across phases (phase S3, found live)

## Problem

The two-phase lifecycle has the **same worker** claim a row **twice**: once `enqueue → queueing`,
then again `in_queue → running`. My first `claim` treated "free to claim" as *unowned OR
lease-expired* — so the second claim, where the row is still owned by that same worker (lease
fresh), was refused. The row stuck at `in_queue`; `resolve` (which requires `running`) then
no-op'd, and nothing reached `state`. Caught by the live `spacetime call` walk-through (R1), not
by compilation.

## Options

- **A · Allow the current owner to re-claim** — `free = unowned || owner == me || lease expired`.
- **B · Separate reducers per phase** — a distinct `claim_execute` that only advances `in_queue
  → running`, so a single "is it mine" check per transition.
- **C · Fold the transition into `stand_up`/`ready`/`resolve`** — no explicit second claim; the
  phase reducers advance status themselves and fence on the worker id.

## Choice — **A**

`free = worker_reference == NONE || worker_reference == me || lease expired`.

## Why

- **A is minimal and correct** — it's the natural fence semantics: a row is claimable by its
  current owner (continue), an unowned row (fresh), or an expired one (eviction/handoff). One
  reducer serves both phase transitions.
- **B** doubles the reducer surface for no real gain.
- **C** couples fencing into every phase reducer and muddies the clean claim→act split.

## Status

Fixed in the pipeline `claim` reducer; re-verified live (event → `complete`, `state` populated).
