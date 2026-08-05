# Version predicates on actions — an action declares which definition versions it understands

_Intent (`server/spacetime/modules/index`, with `docs/ACTIONS.md`). PROPOSED — the user's design,
2026-08-05, captured so the registry is built toward it. **Not built**; the definition registry
([`2026-08-04-definition-registry`](../../../../../../work/2026-08-04-definition-registry/README.md))
is its prerequisite and stops short of it._

## The problem it solves

The registry lets several versions of a definition coexist: a world can hold wolf v0, v1 and v2 at
once, each entity behaving as its own version describes
([F6](../../../../../../work/2026-08-04-definition-registry/forks.md#f6)). Actions, meanwhile, are
written against *a* definition — an action that knows how to shear a v2 sheep may do something
wrong, silently, to a v3 sheep whose fleece works differently.

Coexisting versions without version-aware behaviour is a trap: the data is careful and the code is
not.

## The design

> "I suspect when we write actions we will use `version="<=N"` so that we can block new versions of
> 'wolf' from functioning against un-updated actions. In this way we can, when required, hold an
> action that functions on `<=2` when we implement 3… and we can re-implement the function and hold
> the new action as `==3`. I suspect we would need something like `">= 2 && <= 5"` so we can define
> proper bands." — the user, 2026-08-05

**An action declares the version BAND it understands**, and the dispatcher refuses to run it against
a definition outside that band:

| Predicate | Means |
|---|---|
| `<=2` | this implementation handles everything up to v2 — write it *before* v3 exists, and v3 will not silently fall into it |
| `==3` | a reimplementation, deliberately narrow |
| `>=2 && <=5` | a band, for a run of versions that share a shape |

The pairing is the point: when v3 arrives you keep the old action pinned at `<=2` and add a new one
at `==3`. Old entities keep being handled by the code written for them; new ones get the new code;
neither is a migration.

## Why this shape and not the alternatives

- **Not "the newest action wins."** That is the failure being avoided — it silently applies new
  behaviour to old objects, which is the same mistake as rewriting their ids.
- **Not a version check inside each action body.** It would work, but it is unenforceable: nothing
  makes an author remember, and the failure is silent. A declared predicate is refusable by the
  dispatcher.
- **Not "actions are versioned in lockstep with definitions."** Most revisions do not change what
  an action must do; forcing a new action per version would multiply code for nothing. A band says
  "these versions are the same shape to me", which is exactly the claim an author can make.

## What the registry must provide first

1. **Reverse lookup** — `id → (taxonomy, version)`. A dispatcher receives a `definition_reference`
   and must recover its version to test the predicate. The registry has the mapping; it does not
   yet serve that direction.
2. **Cheap version extraction on the hot path.** The check runs per action, inside the sim. Whether
   that means a resolved-once cache, or the worker holding the LUT, is the first thing this
   stream's successor has to settle — a table lookup per event is not obviously affordable and
   `docs/ACTIONS.md`'s verb dispatch is deliberately tight.

## Open questions for whoever builds it

- **Where the predicate is authored** — on the action in `ACTIONS.md`'s palette (code-owned, like
  the verb arities) or in content beside the definitions it targets.
- **What happens on no match** — reject the action loudly, as the worker already does for an unknown
  `type_id` on `CREATE`, or drop it silently. The `CREATE` precedent argues loudly.
- **Whether predicates compose with the type palette** — an action may care about
  `TYPE_PAWN` broadly and `wolf` versions specifically, which are two different granularities.
