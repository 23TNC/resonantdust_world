# Blockers — texture-restructure

_Things needing human input: what's blocked, why it needs a human, suggested path. Open→resolved;
resolved rows keep a dated resolution line. Goal: fewer over time (a well-understood one becomes a
[`fork`](forks.md) I resolve myself)._

---

## Open

None open. _(B2 was mis-filed here — the registry↔manifest binding is a design decision I resolve,
not a human-input blocker; moved to [`forks.md`](forks.md) F5. See [`issues.md`](issues.md) I2.)_

## Resolved

## Resolved

- **B1 · registry id authority — resolved 2026-07-19, then corrected by [F5](forks.md).** First
  resolved as "registry owns `kind_id`" — **wrong**: tracing the live code showed `kind_id` is the
  **content data DSL's** append-stable first-appearance index ([`loader.rs`](../../../shared/dsl/src/loader.rs)),
  which stored zones depend on. So the registry owns **`form → variant_id` + tile/linked
  classification** (texture-pipeline), and the **data DSL keeps `kind_id`** (never renumber). Superseded
  by F5; keeping the row to record the correction. (This is why you trace the code before authoring ids.)
