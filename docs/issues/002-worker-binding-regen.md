# 002 — Regenerating worker (non-edge) bindings (phase S3)

## Problem

`rd build spacetime shard` runs `generate-bindings.sh`, which emits SDK bindings **only** to
`/workspace/game-server` (host `server/edge`). But the **worker**, **master**, and **npc** each
carry their *own* `src/bindings/shard/` copy, and those are stale after the S2 schema change —
the generator doesn't touch them.

## Options

- **A · Copy the regenerated edge bindings** to the other consumers. They're byte-identical (all
  `spacetimedb_sdk` client bindings for the same module — verified: `diff` of a stable file is
  identical pre-rewrite).
- **B · Extend `generate-bindings.sh`** to emit to every consumer dir (edge + worker + master +
  npc), or loop a consumer list.
- **C · Share one bindings dir** across consumers (a single `shard/` crate they all depend on).

## Choice — **A now, B later**

Copy `server/edge/src/bindings/shard` → the consumer for this phase; note that the generator
should grow a consumer list (B) so this isn't manual.

## Why

- **A is correct and immediate** — the bindings are genuinely identical, so a copy is exactly
  what regenerating would produce; it unblocks S3 without touching build infra mid-rewrite.
- **B is the right permanent fix** but editing `generate-bindings.sh` + the compose wiring is
  infra work orthogonal to the shard rewrite; deferred so it doesn't stall the phase.
- **C** (one shared bindings crate) is cleaner still but a bigger refactor — worth considering
  once the schema stabilizes.

## Status

S3: copied edge → worker bindings. `generate-bindings.sh` extension (B) tracked as follow-up.
