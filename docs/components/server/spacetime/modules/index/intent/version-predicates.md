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

## The band is a LENS, not just a guard — an old action PRODUCES old definitions

> "I suspect we will likely want to use the same versioning when creating items. In theory spawning
> the most recent apple will just work… but we likely need to retain the case that an action using
> old definitions must produce an old definition, as we cannot guarantee the old action to be
> compatible with the new thing." — the user, 2026-08-05

Reading a predicate as an input guard only — *"refuse to run against a v3 sheep"* — is half the
design and the less important half. An action also **creates** things, and an action pinned to `<=2`
that mints the newest apple has produced an object it does not understand.

Concretely: apple v2 weighs 1 and expires in 2 h; apple v3 weighs 2 and expires in 1 h. A `<=2`
recipe computes inventory capacity against weight 1 and sets a 2 h timer. If `CREATE` hands it a v3
apple, the recipe's own arithmetic is wrong **about the thing it just made** — capacity overflows and
the timer outlives the fruit. Nothing rejected anything; the action simply lied.

**So an action resolves names WITHIN its own band.** The predicate is the action's lens on the
registry:

| Context | `resolve("apple")` gives |
|---|---|
| unconstrained code (a fresh player action, worldgen) | the highest version — "spawning the most recent apple just works" |
| an action pinned `<=2` | the highest apple **≤ v2** |
| an action pinned `==3` | apple v3 exactly, or a hard failure if it is gone |

One rule covers both halves: a `<=2` action lives in a v2 world. It refuses v3 inputs *and* produces
v2 outputs, because those are the same claim — "I understand version 2" — read in two directions.

This also makes the pairing from the section above complete. When v3 arrives you keep the old action
at `<=2` and add a new one at `==3`; the old one keeps making v2 apples for as long as it exists,
and the new one makes v3 apples. Neither is a migration, and neither can accidentally handle the
other's fruit.

### The consequence for retirement

[B6](../../../../../../work/2026-08-04-definition-registry/blockers.md#b6) said a version leaves the
corpus once no ENTITY holds it. That is not sufficient. **An action pinned to a version keeps that
version alive even at zero entities** — a `==2` recipe with no v2 apples in the world will mint one
the next time it runs, and it can only do that if v2 is still authored.

So the retirement condition has two clauses, and
[`definition-reclaim.md`](definition-reclaim.md) inherits both:

1. no entity holds the version, **and**
2. no action's band can still resolve to it.

The second is a static check over the action palette rather than a sweep over the world, which makes
it the cheaper of the two — and the one that is easy to forget precisely because it does not involve
any data.

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
- **Whether the band is inherited by nested work.** If a `<=2` recipe queues another action that
  creates something, does the band travel with it? It probably must — otherwise the pin leaks the
  moment an action delegates — but that makes the band part of the event's context rather than a
  property of one instruction.
- **What happens when versions must interact.** A v2 apple and a v3 apple in one inventory: do they
  stack, and if so whose weight applies? Version-aware *creation* does not answer version-aware
  *aggregation*, and the second is where a stacking or crafting system will meet this design.
