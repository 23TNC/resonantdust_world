# content-rollout — pushing a TOML edit into a live world

_User (2026-08-09): "We need to improve our live edit of toml. For example we added the forage
trait to our bunnies but that would require either a reset or more advanced commands to push the
update. So we need a mechanism to force all bunnies from version 1 to version 2 across shards and
ensure that bunny version 2's toml is loaded. This is our simplest case for active rollout. Our
simple case for passive rollout is to wait for all of the bunnies to die and be replaced. For more
complex operations we will require a migration strategy, but for now we are still early in
development where our data is in flux and our state can be reset rather easily."_

The bunny/forager edit is a good specimen because it failed **three times in a row**, at three
different layers, and each failure was silent. Nothing logged. The trait was simply not there.

## Why the forager trait didn't take (surveyed, not assumed)

A TOML edit has to cross three gaps to reach a live bunny. It crosses none of them today.

**1 · The corpus — who is holding which TOML.** Two of five holders hot-swap; three need a restart.

| holder | how it gets the corpus | on an edit |
|---|---|---|
| `server/edge` | polls its source every `RD_CONTENT_POLL_SECS` (10), swaps, bumps the fingerprint; `POST /content/refresh` forces it ([`content.rs`](../../../server/edge/src/content.rs)) | live |
| `client/webgl` | polls `GET /content-version`, re-fetches + hot-swaps ([`contentBoot.ts`](../../../client/webgl/src/game/definitions/contentBoot.ts)) | live |
| `server/worker` | `load_corpus` ONCE at boot from `CONTENT_DIR`, `exit(1)` on failure ([`main.rs:93`](../../../server/worker/src/main.rs), called at :565) | restart |
| `server/master` | reads the tree once at boot to seed the registry ([`main.rs:115`](../../../server/master/src/main.rs)) | restart |
| `client/npc` | `fetch_corpus` once at boot, and each brain then caches `self.def` / `self.kind` / `self.bundle` ([`bunnies.rs:121`](../../../client/npc/src/brains/bunnies.rs)) | restart |

So the worker — the process that *runs* the simulation and composes every mint — was still holding
the pre-edit corpus. That alone is the user's "ensure that bunny version 2's toml is loaded".

**2 · The registry — who has numbered the def.** `index.definitions` already carries a `version`
column, name lookups already resolve to `max(version)` ([`npc/lib.rs:619`](../../../client/npc/src/lib.rs)),
and `ensure_definition` is already idempotent so it can be re-run on every boot
([`index/lib.rs:450`](../../../server/spacetime/server/modules/index/src/lib.rs)). The versioning
half of this stream is **designed and largely built** — [definition-registry](../2026-08-04-definition-registry/README.md)
F3–F7, F12, F17. What is missing is that nothing re-seeds without a master restart, and nothing
notices when a def's *behaviour* changed underneath a version that didn't move.

**3 · The entity — what a live bunny actually stores. This is the one that bit.**
[`mint_sidecars`](../../../server/worker/src/main.rs) (worker `main.rs:155`) writes, at CREATE,
the def's **non-constant** trait binds into the pawn's stored `payload`, plus need rows at the
effective max. [`Bundle::object_trait_rows`](../../../shared/content/src/loader.rs) (`loader.rs:1429`)
then answers "what traits does this object carry" as *the def's CONSTANT binds, derived live from
the corpus with zero storage*, **plus** *the payload's stored rows*.

`forager` is authored under `[[pawn_trait_passive]]` ([`interactions.toml:116`](../../../content/interactions.toml)) —
category 10, not constant. On a bunny minted before the edit it is therefore **neither derived nor
stored**. It does not exist. A bunny minted after the edit has it.

**The load-bearing consequence, stated plainly:** a `[[pawn_trait_constant]]` edit is already live
the moment the corpus reaches the worker, and every other authored field that is *minted* — passive
and active traits, needs, and the effective max those needs mint at — is frozen into each entity at
its birth. **Which of those two you just did is invisible from the TOML**, and nothing tells you.
That asymmetry, not versioning, is what made the forager edit look like it did nothing.

## The stance

Three lanes, strictly ordered, because **no rollout of any kind works until the corpus lane does**
([F1](forks.md#f1)).

- **CORPUS** — every process polls and swaps, the way the browser already does. The edge is the ONE
  source (`GET /content`): it is already the only place that reconciles disk-vs-R2 and already
  strips server-only defs by what the data *is* ([F2](forks.md#f2)). The swap happens **between
  passes**, never inside one — an `Arc<Bundle>` taken at the top of the worker loop, so a single
  tic composes against a single corpus ([F3](forks.md#f3)). The master re-seeds the registry on
  every swap, which `ensure_definition`'s idempotence was written for ([F4](forks.md#f4)).
- **PASSIVE** — drain and replace. Already correct in principle (registry F6: new placements take
  the newest version, old entities keep their id) and already driven by the bunnies' own replenish
  guard. It has exactly one bug: **the brain caches its resolved def at boot**, so it re-mints the
  *old* shape forever. Fix that and the passive lane is real. Give it a counter so it is an
  observable drain rather than a hope ([F9](forks.md#f9)).
- **ACTIVE** — re-stamp in place. `RESTAMP_DEF` (per-entity, writes exactly one entity) fanned out
  by `ROLLOUT_DEF` (writes nothing, queues one per match) — the `BUILD_WALL` verb-that-queues-events
  pattern, chosen because [`ACTIONS.md`](../../ACTIONS.md)'s write-set law makes a broad write set
  the one thing that cannot be sharded away ([F6](forks.md#f6)). Operator-issued via `rd rollout`;
  neither verb ever enters `CLIENT_VERBS` ([F10](forks.md#f10)).

Two decisions carry most of the weight:

- **A version bump is not required to roll out** ([F5](forks.md#f5)). The user's phrasing — "force
  all bunnies from version 1 to version 2" — describes an authored bump, but the edit that prompted
  it was an in-place change at v0. One verb covers both: `RESTAMP_DEF` re-resolves the entity's
  taxonomy to the corpus's newest version, rebinds `definition_reference` if it moved, and re-derives
  the sidecars from whichever def it lands on. Rebinding is a no-op when the id didn't change, so
  in-place edit and authored bump are the same code path.
- **The re-stamp conserves what has a meaning and re-derives what doesn't** ([F7](forks.md#f7)).
  Need *values* survive and get re-clamped to the new bounds; newly authored needs mint at the
  effective max; removed ones drop. Trait rows: the new def's non-constant binds in, the old def's
  out, anything neither def authored left alone (that is `ACTIVATE_TRAIT`'s lane). Conditions,
  position, facing and inventory are untouched. This is deliberately dumb — it is *not* the
  migration strategy the user parked for later — and it is only safe because nothing a player could
  read as a reset gets reset, and because state still resets cheaply.

And one that turns the original silence into a sentence: **staleness becomes visible**
([F8](forks.md#f8)). The registry row gains the corpus's own `sim` hash — [`thing_sim_version`](../../../shared/content/src/loader.rs)
(`loader.rs:1050`) already computes exactly this and nothing consumes it — so the master can report
*"bunny v0: simulation fields changed since the registry recorded it; 12 live entities on the old
shape"*. It **reports, it does not refuse**: refusing is right once data stops being in flux, and
wrong while the user is editing defs hourly.

## Watch

The corpus swap has to be proven not to strand an in-flight event ([I2](issues.md#i2)) — the intent
queue's completions already re-validate ([lumberjack](../2026-08-07-lumberjack/README.md)), so the
expected degradation is a refused act rather than a wrong one, which is a thing to verify rather
than assume. Re-clamping a need **can kill**: the re-stamp feeds the same need-write sweep a
`SET_NEED` feeds, so a def that lowers a `corpus` cap will honestly drop a pawn to a lethal value
([I4](issues.md#i4)) — correct, but it must be expected. An authored version bump burns a `kind_id`
*and* requires the old `[[thing]]` block to stay in the corpus forever (registry F5/F17), which at
this edit cadence is a growing pile of retired blocks — which is the real argument for in-place edit
plus active rollout as the everyday loop, and authored versions only where coexistence is actually
wanted ([I3](issues.md#i3)). Ordering is load-bearing in two places: the master must re-seed
*before* the worker's `definitions` mirror is read for a rollout ([I8](issues.md#i8)), the same
order the browser already takes deliberately. And the worker must be confirmed not to need the
server-only `[[biome]]` blocks that `/content` strips, before it is repointed there
([I1](issues.md#i1)).

## Not in this stream (named, not built)

The **migration strategy** the user explicitly deferred — per-field rules for the hard cases
registry F6 enumerated (the apple with 30 minutes left, the inventory now over capacity). Id
**reclaim** (registry F7). **Cold rollout**: this stream is pawns; rolling a `biome-thing` or
`biome-tile` version is a `SET` sweep over cold rows and is the named successor. Per-package
content versioning. Removing the `--reset` republish, which stays the tool for *schema* changes —
this stream only makes it unnecessary for *content* changes ([I9](issues.md#i9)).

**Version predicates** — the user's 2026-08-05 design where an action declares the version *band* it
understands (`<=2`, `==3`) so old code keeps handling old entities, captured at
[`version-predicates.md`](../../components/server/spacetime/modules/index/intent/version-predicates.md).
That is the complement of this stream, not a part of it: predicates make coexisting versions safe to
*leave alone*, rollout is for when you want them gone. Both stand on the same registry, and they
compose — a rolled-out world simply has fewer bands in play.

## Exit

Save an edit to [`things.toml`](../../../content/things.toml) and, within one poll interval and with
no process restarted, the worker, master, npc and browser all hold it. `rd rollout --status bunny`
prints the live population split by def id with the stale ones flagged. `rd rollout bunny` puts the
forage intent on every live bunny within a tic or two, with hunger and thirst values preserved and
no death the values didn't already imply. Killing a bunny instead re-mints the new shape without an
npc restart, and the status counter drains to zero on its own. The user's eyes close the stream.
