# blockers — login-without-spacetime

Things needing human input: what blocks, *why* it needs a human, the options, and a suggested path.
Open→resolved; resolved rows archive with a date.

A **fork is mine to resolve; a blocker needs the user.** Filing a decision I could make is a
manufactured pause — those go in [`forks.md`](forks.md).

_None. Every decision this stream needed is resolved in [`forks.md`](forks.md) F1–F8._

## Not blockers, but flagged for awareness

Recorded here because they need to be *seen*, not because they stop work:

- **TOFU is localhost-only.** Anyone reaching `/login` can claim any unclaimed name, `Developer`
  included. This must be closed (forks F2, option 3 — a per-player secret at create time) before the
  gateway is reachable from outside. It does not block this stream.
- **Removing SpacetimeDB removes the world-state store and the push fan, not just login.** This
  stream fills neither hole, and login needs neither. Those are the next two design problems and
  they are larger than this one.
