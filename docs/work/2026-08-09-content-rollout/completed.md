# Completed — content-rollout

_The verification log: dated entries saying what landed and **how it was checked**. Append-only;
authoritative for what is done and why we believe it. Items live in [`todo.md`](todo.md) with their
boxes ticked — this file records evidence, not item text._

## 2026-08-09 — the stream opened

The survey that produced [`README.md`](README.md), run before any code was written. What was
checked, and how:

- **Corpus holders enumerated by reading each load site**, not inferred: the edge polls and swaps
  ([`server/edge/src/content.rs`](../../../server/edge/src/content.rs), `RD_CONTENT_POLL_SECS`
  default 10 at [`config.rs:24`](../../../server/edge/src/config.rs)); the browser polls
  `/content-version` ([`contentBoot.ts`](../../../client/webgl/src/game/definitions/contentBoot.ts));
  the worker loads once at [`main.rs:93`](../../../server/worker/src/main.rs) called from `:565`;
  the master seeds once at [`main.rs:115`](../../../server/master/src/main.rs); the npc fetches once
  at [`lib.rs:562`](../../../client/npc/src/lib.rs).
- **The forager failure traced to the mint, not to versioning.** `forager` is authored under
  `[[pawn_trait_passive]]` ([`interactions.toml:116`](../../../content/interactions.toml)), which
  maps to category 10 (`GAMEPLAY_PAWN_TRAIT_PASSIVE`, [`codec/object.rs:111`](../../../shared/codec/src/object.rs)).
  [`object_trait_rows`](../../../shared/content/src/loader.rs) (`loader.rs:1429`) derives only the
  `*_constant` categories live and takes everything else from stored payload rows;
  [`mint_sidecars`](../../../server/worker/src/main.rs) (`worker/main.rs:155`) writes those stored
  rows at CREATE only. A bunny minted before the edit therefore carries the trait in neither lane.
- **The passive lane's own blocker found by reading the brain**, not by running it:
  [`bunnies.rs:121`](../../../client/npc/src/brains/bunnies.rs) caches `self.def` / `self.kind` /
  `self.bundle` at corpus load, so the replenish guard re-mints the old def indefinitely.
- **The versioning half confirmed already built**: `index.definitions` carries `version`
  ([`index/lib.rs:450`](../../../server/spacetime/server/modules/index/src/lib.rs)),
  `ensure_definition` is idempotent-or-collide, name lookups already take `max(version)`
  ([`npc/lib.rs:619`](../../../client/npc/src/lib.rs)), and the loader already computes an unused
  simulation-visible hash at [`loader.rs:1050`](../../../shared/content/src/loader.rs). Two authored
  versions of one tuple are proven to coexist by an existing test in
  [`server/master/src/defs.rs`](../../../server/master/src/defs.rs)
  (`each_authored_version_gets_its_own_row_at_its_own_position`).
- **Verb numbers checked against the palette**: 19 is `ACTIVATE_TRAIT`
  ([`action.rs:145`](../../../shared/codec/src/action.rs)), so 20 and 21 are the next free.
